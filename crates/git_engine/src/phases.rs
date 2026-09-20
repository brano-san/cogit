use std::time::{Duration, Instant};

/// The phases `git --progress` announces, in the order a transfer walks through them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    /// Connection, authentication and ref advertisement: git prints nothing while it waits.
    Negotiating,
    Enumerating,
    Counting,
    Compressing,
    Receiving,
    Writing,
    Resolving,
    UpdatingTree,
}

impl Phase {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Negotiating => "negotiating",
            Self::Enumerating => "enumerating",
            Self::Counting => "counting",
            Self::Compressing => "compressing",
            Self::Receiving => "receiving",
            Self::Writing => "writing",
            Self::Resolving => "resolving",
            Self::UpdatingTree => "updating-tree",
        }
    }

    /// Who the user would have to talk to about time spent here.
    fn owner(self) -> &'static str {
        match self {
            Self::Negotiating => "connection setup",
            Self::Enumerating | Self::Counting | Self::Compressing => "remote server",
            Self::Receiving | Self::Writing => "network transfer",
            Self::Resolving => "local delta resolution",
            Self::UpdatingTree => "local working tree update",
        }
    }
}

#[must_use]
pub fn phase_of(line: &str) -> Option<Phase> {
    let body = line.strip_prefix("remote:").unwrap_or(line).trim_start();
    let label = body.split(':').next()?.trim();
    match label {
        "Enumerating objects" => Some(Phase::Enumerating),
        "Counting objects" => Some(Phase::Counting),
        "Compressing objects" | "Delta compression using up to 8 threads" => {
            Some(Phase::Compressing)
        }
        "Receiving objects" | "Unpacking objects" => Some(Phase::Receiving),
        "Writing objects" => Some(Phase::Writing),
        "Resolving deltas" => Some(Phase::Resolving),
        "Updating files" | "Checking out files" => Some(Phase::UpdatingTree),
        _ => None,
    }
}

#[derive(Debug, Clone, Default)]
pub struct Timings {
    pub total: Duration,
    phases: Vec<(Phase, Duration)>,
    pub bytes: Option<u64>,
    pub objects: Option<u64>,
}

impl Timings {
    #[must_use]
    pub fn of(&self, phase: Phase) -> Duration {
        self.phases
            .iter()
            .find(|(seen, _)| *seen == phase)
            .map_or(Duration::ZERO, |(_, elapsed)| *elapsed)
    }

    /// `receiving=300` pairs, ordered by phase, for one grep-able log line.
    #[must_use]
    pub fn summary(&self) -> String {
        self.phases
            .iter()
            .filter(|(_, elapsed)| !elapsed.is_zero())
            .map(|(phase, elapsed)| format!("{}={}", phase.name(), elapsed.as_millis()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Bytes per second over the transferring phases, when git reported a size.
    #[must_use]
    pub fn transfer_rate(&self) -> Option<f64> {
        let moving = self.of(Phase::Receiving) + self.of(Phase::Writing);
        let seconds = moving.as_secs_f64();
        if seconds <= 0.0 {
            return None;
        }
        Some(self.bytes? as f64 / seconds)
    }
}

/// What to blame for the wall-clock time: the phase that took the longest owns it.
#[must_use]
pub fn bottleneck(timings: &Timings) -> &'static str {
    timings
        .phases
        .iter()
        .max_by_key(|(_, elapsed)| *elapsed)
        .map_or("connection setup", |(phase, _)| phase.owner())
}

/// Turns a stream of `--progress` lines into per-phase durations.
#[derive(Debug)]
pub struct PhaseTimer {
    started: Instant,
    current: Phase,
    since: Duration,
    totals: Vec<(Phase, Duration)>,
    bytes: Option<u64>,
    objects: Option<u64>,
}

impl Default for PhaseTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseTimer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            current: Phase::Negotiating,
            since: Duration::ZERO,
            totals: Vec::new(),
            bytes: None,
            objects: None,
        }
    }

    pub fn observe(&mut self, line: &str) {
        self.observe_at(line, self.started.elapsed());
    }

    /// The clock is a parameter so the phase arithmetic is testable without sleeping.
    pub fn observe_at(&mut self, line: &str, at: Duration) {
        if let Some(count) = objects_of(line) {
            self.objects = Some(count);
        }
        if let Some(size) = bytes_of(line) {
            self.bytes = Some(size);
        }

        let Some(phase) = phase_of(line) else { return };
        if phase == self.current {
            return;
        }
        self.close(at);
        self.current = phase;
    }

    pub fn finish(self) -> Timings {
        let at = self.started.elapsed();
        self.finish_at(at)
    }

    #[must_use]
    pub fn finish_at(mut self, at: Duration) -> Timings {
        self.close(at);
        self.totals.sort_by_key(|(phase, _)| *phase);
        Timings {
            total: at,
            phases: self.totals,
            bytes: self.bytes,
            objects: self.objects,
        }
    }

    fn close(&mut self, at: Duration) {
        let elapsed = at.saturating_sub(self.since);
        match self
            .totals
            .iter_mut()
            .find(|(seen, _)| *seen == self.current)
        {
            Some((_, total)) => *total += elapsed,
            None => self.totals.push((self.current, elapsed)),
        }
        self.since = at;
    }
}

fn objects_of(line: &str) -> Option<u64> {
    let open = line.find('(')?;
    let close = line[open..].find(')')? + open;
    let (_, total) = line[open + 1..close].split_once('/')?;
    total.trim().parse().ok()
}

/// `3.50 MiB` in a progress line is what actually crossed the wire.
fn bytes_of(line: &str) -> Option<u64> {
    let (before, unit) = UNITS
        .iter()
        .find_map(|(suffix, scale)| line.split_once(suffix).map(|(head, _)| (head, *scale)))?;
    let number: f64 = before
        .rsplit(|c: char| c != '.' && !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    Some((number * unit) as u64)
}

const UNITS: &[(&str, f64)] = &[
    (" GiB", 1_073_741_824.0),
    (" MiB", 1_048_576.0),
    (" KiB", 1024.0),
    (" bytes", 1.0),
];
