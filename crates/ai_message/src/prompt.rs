use std::fmt::Write as _;

pub const DIFF_BUDGET: usize = 40_000;
const HEADER_BUDGET: usize = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skipped {
    Binary,
    Lockfile,
    OverBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub path: String,
    pub old_path: Option<String>,
    pub added: Option<u32>,
    pub deleted: Option<u32>,
    pub skipped: Option<Skipped>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Staged {
    pub branch: Option<String>,
    pub subjects: Vec<String>,
    pub changes: Vec<Change>,
    pub diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    pub system: String,
    pub user: String,
    pub shortened: bool,
}

const SYSTEM: &str = "You write the commit message for the staged changes of a git repository.
Reply with the commit message only: no introduction, no explanation, no quotes, no Markdown code fences.
The first line is a summary in the imperative mood, at most 72 characters. If the change needs more explanation, add a blank line and a short body that says what changed and why, wrapped at 72 characters. Do not list every file.
Follow the style of the recent commit subjects: their language, prefixes such as Conventional Commits types and scopes, and capitalization.
Some file contents may be left out of the diff; describe those files from the list of staged files.";

#[must_use]
pub fn build(staged: &Staged, prefix: Option<&str>, instructions: &str) -> Prompt {
    build_with_budget(staged, prefix, instructions, DIFF_BUDGET)
}

#[must_use]
pub fn build_with_budget(
    staged: &Staged,
    prefix: Option<&str>,
    instructions: &str,
    budget: usize,
) -> Prompt {
    let mut system = SYSTEM.to_owned();
    if let Some(prefix) = prefix.filter(|prefix| !prefix.trim().is_empty()) {
        let _ = write!(
            system,
            "\nBegin the first line with exactly this text: {prefix}"
        );
    }
    let instructions = instructions.trim();
    if !instructions.is_empty() {
        let _ = write!(
            system,
            "\n\nFurther instructions from the user:\n{instructions}"
        );
    }

    let (diff, shown, total) = shorten(&staged.diff, budget);
    let left_out = staged
        .changes
        .iter()
        .any(|change| matches!(change.skipped, Some(Skipped::OverBudget)));
    let shortened = diff.len() < staged.diff.len() || left_out;

    let mut user = String::new();
    let _ = writeln!(
        user,
        "Branch: {}",
        staged.branch.as_deref().unwrap_or("(detached HEAD)")
    );
    if !staged.subjects.is_empty() {
        user.push_str("\nRecent commit subjects in this repository, newest first:\n");
        for subject in &staged.subjects {
            let _ = writeln!(user, "- {subject}");
        }
    }
    user.push_str("\nStaged files:\n");
    for change in &staged.changes {
        let _ = writeln!(user, "- {}", file_line(change));
    }
    if shortened {
        let _ = write!(
            user,
            "\nNote: the diff below was shortened to keep the request small: {shown} of {total} complete hunks are shown"
        );
        user.push_str(if left_out {
            ", and the files marked \"not shown\" are left out.\n"
        } else {
            ".\n"
        });
    }
    user.push_str("\nStaged diff (git diff --cached):\n");
    user.push_str(&diff);
    Prompt {
        system,
        user,
        shortened,
    }
}

fn file_line(change: &Change) -> String {
    let mut line = change.path.clone();
    if let Some(old) = &change.old_path {
        let _ = write!(line, " (renamed from {old})");
    }
    if let (Some(added), Some(deleted)) = (change.added, change.deleted) {
        let _ = write!(line, " +{added} -{deleted}");
    }
    let why = match change.skipped {
        None => return line,
        Some(Skipped::Binary) => "binary",
        Some(Skipped::Lockfile) => "lockfile",
        Some(Skipped::OverBudget) => "over the size limit of the request",
    };
    let _ = write!(line, " ({why}, contents not shown)");
    line
}

struct FileDiff<'a> {
    header: &'a str,
    hunks: Vec<&'a str>,
}

fn split_files(diff: &str) -> Vec<FileDiff<'_>> {
    let mut starts: Vec<usize> = Vec::new();
    let mut at = 0;
    for line in diff.split_inclusive('\n') {
        if line.starts_with("diff --git ") || (starts.is_empty() && at == 0) {
            starts.push(at);
        }
        at += line.len();
    }
    starts.dedup();
    let mut files = Vec::new();
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(diff.len());
        let text = &diff[start..end];
        if text.is_empty() {
            continue;
        }
        let mut hunk_starts = Vec::new();
        let mut offset = 0;
        for line in text.split_inclusive('\n') {
            if line.starts_with("@@") {
                hunk_starts.push(offset);
            }
            offset += line.len();
        }
        let header_end = hunk_starts.first().copied().unwrap_or(text.len());
        let hunks = hunk_starts
            .iter()
            .enumerate()
            .map(|(i, &s)| &text[s..hunk_starts.get(i + 1).copied().unwrap_or(text.len())])
            .collect();
        files.push(FileDiff {
            header: &text[..header_end],
            hunks,
        });
    }
    files
}

fn cut_lines(text: &str, room: usize) -> &str {
    let mut end = 0;
    for line in text.split_inclusive('\n') {
        if end + line.len() > room {
            break;
        }
        end += line.len();
    }
    &text[..end]
}

#[must_use]
pub fn shorten(diff: &str, budget: usize) -> (String, usize, usize) {
    let files = split_files(diff);
    let total: usize = files.iter().map(|file| file.hunks.len()).sum();
    if diff.len() <= budget {
        return (diff.to_owned(), total, total);
    }
    let mut room = budget;
    let headers: Vec<&str> = files
        .iter()
        .map(|file| {
            let header = cut_lines(file.header, room.min(HEADER_BUDGET));
            room -= header.len();
            header
        })
        .collect();
    let mut order: Vec<usize> = (0..files.len()).collect();
    order.sort_by_cached_key(|&index| {
        std::cmp::Reverse(
            files[index]
                .hunks
                .iter()
                .flat_map(|hunk| hunk.lines().filter(|line| line.starts_with(['+', '-'])))
                .count(),
        )
    });
    let mut taken = vec![Vec::new(); files.len()];
    let mut shown = 0;
    for index in order {
        if headers[index].is_empty() {
            continue;
        }
        for hunk in &files[index].hunks {
            let part = cut_lines(hunk, room);
            if part.lines().nth(1).is_none() {
                break;
            }
            room -= part.len();
            shown += usize::from(part.len() == hunk.len());
            taken[index].push(part);
            if part.len() < hunk.len() {
                break;
            }
        }
    }
    let mut out = String::with_capacity(budget - room);
    for (header, hunks) in headers.into_iter().zip(taken) {
        out.push_str(header);
        for hunk in hunks {
            out.push_str(hunk);
        }
    }
    (out, shown, total)
}

#[must_use]
pub fn clean_message(text: &str) -> String {
    let mut text = text.trim();
    if let Some(inner) = text.strip_prefix("```") {
        let inner = inner.split_once('\n').map_or("", |(_, rest)| rest);
        text = inner.trim_end().strip_suffix("```").unwrap_or(inner).trim();
    }
    text.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hunk(n: usize, lines: usize) -> String {
        let mut text = format!("@@ -{n},{lines} +{n},{lines} @@\n");
        for i in 0..lines {
            let _ = writeln!(text, "+line {n}.{i} of some length to fill");
        }
        text
    }

    fn file(path: &str, hunks: &[String]) -> String {
        let mut text = format!(
            "diff --git a/{path} b/{path}\nindex 1..2 100644\n--- a/{path}\n+++ b/{path}\n"
        );
        for each in hunks {
            text.push_str(each);
        }
        text
    }

    #[test]
    fn a_small_diff_goes_whole() {
        let diff = file("a.rs", &[hunk(1, 3)]);
        assert_eq!(shorten(&diff, 10_000), (diff.clone(), 1, 1));
    }

    #[test]
    fn the_most_changed_file_gets_hunks_first() {
        let big: Vec<String> = (0..20).map(|n| hunk(n * 100, 40)).collect();
        let diff = [file("small.rs", &[hunk(1, 2)]), file("big.rs", &big)].concat();
        let (text, shown, total) = shorten(&diff, 6_000);
        assert_eq!(total, 21);
        assert!(shown < total);
        assert!(text.len() <= 6_000);
        assert!(text.contains("diff --git a/small.rs b/small.rs"));
        assert!(!text.contains("+line 1.0"));
        assert!(text.contains("+line 200.0"));
        let small = text.find("small.rs").unwrap_or(0);
        assert!(small > 0);
    }

    #[test]
    fn a_huge_first_hunk_is_cut_at_a_line() {
        let diff = file("one.rs", &[hunk(1, 2_000)]);
        let (text, shown, _) = shorten(&diff, 3_000);
        assert_eq!(shown, 0);
        assert!(text.len() <= 3_000);
        assert!(text.ends_with("of some length to fill\n"));
    }

    fn staged(diff: String, changes: Vec<Change>) -> Staged {
        Staged {
            branch: Some("feature/ABC-1".into()),
            subjects: vec!["feat(ui): add a button".into(), "fix: crash".into()],
            changes,
            diff,
        }
    }

    fn change(path: &str, skipped: Option<Skipped>) -> Change {
        Change {
            path: path.into(),
            old_path: None,
            added: Some(1),
            deleted: Some(0),
            skipped,
        }
    }

    #[test]
    fn the_prompt_carries_branch_style_files_and_prefix() {
        let prompt = build(
            &staged(
                file("a.rs", &[hunk(1, 2)]),
                vec![
                    change("a.rs", None),
                    change("Cargo.lock", Some(Skipped::Lockfile)),
                ],
            ),
            Some("ABC-1: "),
            "Write in German.",
        );
        assert!(
            prompt
                .system
                .contains("Begin the first line with exactly this text: ABC-1: ")
        );
        assert!(prompt.system.ends_with("Write in German."));
        assert!(prompt.user.contains("Branch: feature/ABC-1"));
        assert!(prompt.user.contains("- feat(ui): add a button"));
        assert!(
            prompt
                .user
                .contains("- Cargo.lock +1 -0 (lockfile, contents not shown)")
        );
        assert!(prompt.user.contains("+line 1.0"));
        assert!(!prompt.shortened);
        assert!(!prompt.user.contains("Note: the diff"));
    }

    #[test]
    fn a_shortened_diff_says_so() {
        let big: Vec<String> = (0..50).map(|n| hunk(n * 100, 40)).collect();
        let prompt = build_with_budget(
            &staged(file("big.rs", &big), vec![change("big.rs", None)]),
            None,
            "",
            4_000,
        );
        assert!(prompt.shortened);
        assert!(prompt.user.contains("Note: the diff below was shortened"));
        assert!(!prompt.system.contains("Begin the first line"));
        let left = build(
            &staged(
                String::new(),
                vec![change("x.rs", Some(Skipped::OverBudget))],
            ),
            None,
            "",
        );
        assert!(left.shortened);
        assert!(left.user.contains("not shown\" are left out"));
    }

    #[test]
    fn fences_and_blank_lines_around_the_answer_go() {
        assert_eq!(
            clean_message("```\nfix: a\n\nbody\n```\n"),
            "fix: a\n\nbody"
        );
        assert_eq!(clean_message("```text\nfix: a\n```"), "fix: a");
        assert_eq!(clean_message("\n fix: b \n\n"), "fix: b");
    }
}
