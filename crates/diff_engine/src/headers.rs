use crate::FileDiff;

const MAX_CONTEXT: usize = 80;

/// Git's own rule, not a parser (doc/12-risks.md, R-28).
pub fn with_hunk_context(diff: &mut FileDiff, old_text: &str) {
    let FileDiff::Text { hunks, .. } = diff else {
        return;
    };
    // One pass over the lines for all hunks, which come in order: the last line that
    // declares something above each is the one carried along so far.
    let mut lines = old_text.lines();
    let (mut read, mut context) = (0, None);
    for hunk in hunks.iter_mut() {
        // An empty old side starts at the line it follows, which is above the change too.
        let above = if hunk.old_lines == 0 {
            hunk.old_start
        } else {
            hunk.old_start.saturating_sub(1)
        } as usize;
        if above < read {
            (lines, read, context) = (old_text.lines(), 0, None);
        }
        while read < above {
            let Some(line) = lines.next() else { break };
            read += 1;
            if declares(line) {
                context = Some(line);
            }
        }
        let Some(context) = context else {
            continue;
        };
        hunk.header.push(' ');
        hunk.header
            .extend(context.trim_end().chars().take(MAX_CONTEXT));
    }
}

/// C++ puts these unindented inside a class body, so the generic rule below picks one up
/// and the header reads `public:`, which tells the reader nothing about where they are.
/// Matched whole rather than by the trailing colon: Python's `class Foo:` must survive.
const ACCESS_SPECIFIERS: &[&str] = &[
    "public:",
    "private:",
    "protected:",
    "signals:",
    "public slots:",
    "private slots:",
    "protected slots:",
];

fn declares(line: &str) -> bool {
    let starts = line
        .chars()
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$');

    starts && !ACCESS_SPECIFIERS.contains(&line.trim_end())
}
