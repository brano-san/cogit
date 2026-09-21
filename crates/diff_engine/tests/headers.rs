// clippy.toml's allow-unwrap-in-tests does not reach helpers beside `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use diff_engine::{DiffOptions, FileDiff, diff_text, with_hunk_context};

fn headers(old: &str, new: &str) -> Vec<String> {
    let mut diff = diff_text(old, new, &DiffOptions::default());
    with_hunk_context(&mut diff, old);
    match diff {
        FileDiff::Text { hunks, .. } => hunks.into_iter().map(|h| h.header).collect(),
        other => panic!("expected a text diff, got {other:?}"),
    }
}

/// `count` indented statements, enough that a hunk in the middle has the declaration
/// well above its context.
fn prefixed(prefix: &str, count: usize) -> String {
    (0..count)
        .map(|i| {
            format!(
                "    let {prefix}{i} = {i};
"
            )
        })
        .collect()
}

fn statements(count: usize) -> String {
    (0..count)
        .map(|i| format!("    let v{i} = {i};\n"))
        .collect()
}

fn function(name: &str, count: usize) -> String {
    format!("fn {name}() {{\n{}}}\n", prefixed(&name[..1], count))
}

#[test]
fn the_enclosing_function_is_appended_to_the_header() {
    let old = function("outer", 12);
    let new = old.replace("let o6 = 6;", "let o6 = 66;");

    assert!(
        headers(&old, &new)[0].ends_with("@@ fn outer() {"),
        "{:?}",
        headers(&old, &new)
    );
}

#[test]
fn the_counts_in_front_are_left_alone() {
    let old = function("outer", 12);
    let new = old.replace("let o6 = 6;", "let o6 = 66;");

    assert!(
        headers(&old, &new)[0].starts_with("@@ -5,7 +5,7 @@"),
        "{:?}",
        headers(&old, &new)
    );
}

#[test]
fn a_change_with_nothing_declared_above_it_gets_no_context() {
    let old = statements(12);
    let new = old.replace("let v6 = 6;", "let v6 = 66;");

    assert_eq!(headers(&old, &new)[0], "@@ -4,7 +4,7 @@");
}

#[test]
fn the_nearest_declaration_above_wins() {
    let old = format!("{}{}", function("first", 12), function("second", 12));
    let new = old.replace("let f6 = 6;", "let f6 = 66;");

    assert!(
        headers(&old, &new)[0].ends_with("fn first() {"),
        "{:?}",
        headers(&old, &new)
    );
}

#[test]
fn each_hunk_gets_the_declaration_that_encloses_it() {
    let old = format!("{}{}", function("first", 12), function("second", 12));
    let new = old
        .replace("let f6 = 6;", "let f6 = 66;")
        .replace("let s6 = 6;", "let s6 = 66;");

    let headers = headers(&old, &new);

    assert!(headers.len() >= 2, "{headers:?}");
    assert!(headers[0].ends_with("fn first() {"), "{headers:?}");
    assert!(
        headers.last().unwrap().ends_with("fn second() {"),
        "{headers:?}"
    );
}

#[test]
fn an_indented_line_is_not_taken_for_a_declaration() {
    let old = format!(
        "fn outer() {{\n    if ready {{\n{}    }}\n}}\n",
        statements(12)
    );
    let new = old.replace("let v6 = 6;", "let v6 = 66;");

    assert!(
        headers(&old, &new)[0].ends_with("fn outer() {"),
        "an indented `if` is not a declaration: {:?}",
        headers(&old, &new)
    );
}

#[test]
fn a_declaration_may_start_with_an_underscore() {
    let old = format!("_private() {{\n{}}}\n", statements(12));
    let new = old.replace("let v6 = 6;", "let v6 = 66;");

    assert!(
        headers(&old, &new)[0].ends_with("_private() {"),
        "{:?}",
        headers(&old, &new)
    );
}

#[test]
fn a_header_is_never_longer_than_the_terminal_can_show() {
    let long = "x".repeat(400);
    let old = format!("fn {long}() {{\n{}}}\n", statements(12));
    let new = old.replace("let v6 = 6;", "let v6 = 66;");

    assert!(headers(&old, &new)[0].len() < 200);
}

#[test]
fn a_diff_that_is_not_text_is_left_untouched() {
    let mut diff = FileDiff::Unchanged;
    with_hunk_context(&mut diff, "");
    assert!(matches!(diff, FileDiff::Unchanged));
}

/// C++ free function: `with_hunk_context` has no grammar, it looks for the nearest
/// unindented line, and a C++ signature is one (doc/08-diff-engine.md).
#[test]
fn a_change_in_a_cpp_function_body_names_that_function() {
    let body: String = (0..12).map(|i| format!("    int v{i} = {i};\n")).collect();
    let old = format!("#include <vector>\n\nvoid Widget::draw(int x) {{\n{body}}}\n");
    let new = old.replace("int v6 = 6;", "int v6 = 66;");

    let header = headers(&old, &new).remove(0);
    assert!(
        header.ends_with("@@ void Widget::draw(int x) {"),
        "{header:?}"
    );
}

/// A method declared inside a class body is indented, so the nearest unindented line is
/// the class. That is the rule working as designed, not a miss: the class still tells the
/// reader where they are.
#[test]
fn a_change_inside_a_cpp_class_method_names_the_class() {
    let body: String = (0..12)
        .map(|i| format!("        int v{i} = {i};\n"))
        .collect();
    let old = format!("class Widget {{\npublic:\n    void draw(int x) {{\n{body}    }}\n}};\n");
    let new = old.replace("int v6 = 6;", "int v6 = 66;");

    let header = headers(&old, &new).remove(0);
    assert!(header.ends_with("@@ class Widget {"), "{header:?}");
}
