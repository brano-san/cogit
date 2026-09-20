use crate::FileDiff;

const MAX_CONTEXT: usize = 80;

/// Git's own rule, not a parser (doc/12-risks.md, R-28).
pub fn with_hunk_context(diff: &mut FileDiff, old_text: &str) {
    let FileDiff::Text { hunks, .. } = diff else {
        return;
    };
    let lines: Vec<&str> = old_text.lines().collect();

    for hunk in hunks.iter_mut() {
        let above = hunk.old_start.saturating_sub(1) as usize;
        let Some(context) = lines[..above.min(lines.len())]
            .iter()
            .rev()
            .find(|line| declares(line))
        else {
            continue;
        };
        let trimmed: String = context.trim_end().chars().take(MAX_CONTEXT).collect();
        hunk.header = format!("{} {trimmed}", hunk.header);
    }
}

fn declares(line: &str) -> bool {
    line.chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
}
