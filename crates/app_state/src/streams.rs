use crate::backend::{Remote, Request, answer, to_raw};
use crate::graph_cache::GraphProgress;
use crate::{AppState, RepoId, WorktreeScanChunk};
use git_engine::{CommitQuery, GitError, OriginQuery, OriginReport, SearchScope, SkippedRef};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cell::Cell;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum StreamCall {
    BuildGraph {
        repo: RepoId,
        query: CommitQuery,
        generation: u32,
        chunk_size: usize,
    },
    SearchContents {
        repo: RepoId,
        query: String,
        is_regex: bool,
        scope: SearchScope,
    },
    OriginCandidates {
        repo: RepoId,
        query: OriginQuery,
    },
    ScanWorktreeRemoval {
        repo: RepoId,
        path: String,
    },
}

impl StreamCall {
    pub(crate) fn run(
        self,
        state: &AppState,
        call: &agent::Call,
    ) -> Result<agent::Json, agent::Json> {
        let stopped = || call.stop().is_stopped();
        match self {
            Self::BuildGraph {
                repo,
                query,
                generation,
                chunk_size,
            } => {
                state.adopt_graph_generation(generation);
                answer(
                    state.build_graph(repo, &query, generation, chunk_size, |progress| {
                        call.chunk(to_raw(&progress)) && !stopped()
                    }),
                )
            }
            Self::SearchContents {
                repo,
                query,
                is_regex,
                scope,
            } => {
                let request = git_engine::SearchRequest {
                    query: &query,
                    is_regex,
                    scope,
                };
                answer(
                    state.search_contents(repo, &request, &stopped, &mut |batch| {
                        call.chunk(to_raw(&batch));
                    }),
                )
            }
            Self::OriginCandidates { repo, query } => {
                answer(state.origin_candidates(repo, &query, &stopped))
            }
            Self::ScanWorktreeRemoval { repo, path } => {
                let stop = call.stop().clone();
                answer(state.scan_worktree_removal(
                    repo,
                    &path,
                    move || stop.is_stopped(),
                    |chunk| {
                        call.chunk(to_raw(&chunk));
                    },
                ))
            }
        }
    }
}

fn each<T: DeserializeOwned>(mut take: impl FnMut(T)) -> impl FnMut(agent::Json) {
    move |value| match serde_json::from_str(value.get()) {
        Ok(item) => take(item),
        Err(err) => tracing::error!(error = ?err, context = "a chunk from the agent"),
    }
}

impl AppState {
    pub(crate) fn graph_through(
        &self,
        remote: &Remote,
        query: &CommitQuery,
        generation: u32,
        chunk_size: usize,
        on_progress: &mut dyn FnMut(GraphProgress) -> bool,
    ) -> Result<Vec<SkippedRef>, GitError> {
        let ended = Cell::new(false);
        let request = Request::Stream(StreamCall::BuildGraph {
            repo: remote.id(),
            query: query.clone(),
            generation,
            chunk_size,
        });
        remote.stream(
            &request,
            &|| ended.get(),
            &mut each(|progress| {
                if ended.get() || !self.is_current_graph(generation) || !on_progress(progress) {
                    ended.set(true);
                }
            }),
        )
    }

    pub(crate) fn search_through(
        &self,
        remote: &Remote,
        request: &git_engine::SearchRequest<'_>,
        cancelled: &dyn Fn() -> bool,
        on_batch: &mut dyn FnMut(Vec<git_engine::ContentMatch>),
    ) -> Result<(), GitError> {
        let request = Request::Stream(StreamCall::SearchContents {
            repo: remote.id(),
            query: request.query.to_owned(),
            is_regex: request.is_regex,
            scope: request.scope,
        });
        remote.stream(&request, cancelled, &mut each(on_batch))
    }

    pub(crate) fn origins_through(
        &self,
        remote: &Remote,
        query: &OriginQuery,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Option<OriginReport>, GitError> {
        let request = Request::Stream(StreamCall::OriginCandidates {
            repo: remote.id(),
            query: query.clone(),
        });
        remote.stream(&request, cancelled, &mut |_| {})
    }

    pub(crate) fn scan_through(
        &self,
        remote: &Remote,
        path: &str,
        stop: &dyn Fn() -> bool,
        send: &dyn Fn(WorktreeScanChunk),
    ) -> Result<(), GitError> {
        let request = Request::Stream(StreamCall::ScanWorktreeRemoval {
            repo: remote.id(),
            path: path.to_owned(),
        });
        remote.stream(
            &request,
            stop,
            &mut each(|chunk| {
                if !stop() {
                    send(chunk);
                }
            }),
        )
    }
}
