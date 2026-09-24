#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Syntactic 3-way merge (doc/08-diff-engine.md §8). Two sides that edited different
//! functions do not need the user, even when their line ranges abut. Two sides that
//! edited the same function always do: the rule is certainty, not cleverness.

use diff_engine::{Origin, Region, merge3, merge3_with_syntax};

const BASE: &str = "\
int first() {
    return 1;
}

int second() {
    return 2;
}
";

fn kinds(regions: &[Region]) -> Vec<&'static str> {
    regions
        .iter()
        .map(|region| match region {
            Region::Clean {
                origin: Origin::Syntactic,
                ..
            } => "syntactic",
            Region::Clean { .. } => "clean",
            Region::Conflict { .. } => "conflict",
        })
        .collect()
}

fn text(regions: &[Region]) -> String {
    regions
        .iter()
        .flat_map(|region| match region {
            Region::Clean { lines, .. } => lines.clone(),
            Region::Conflict { base, .. } => base.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn edits_to_different_functions_are_settled_without_the_user() {
    let ours = BASE.replace("return 1;", "return 11;");
    let theirs = BASE.replace("return 2;", "return 22;");

    let merged = merge3_with_syntax(BASE, &ours, &theirs, Some("cpp"));

    assert!(
        !merged.iter().any(Region::is_conflict),
        "{:?}",
        kinds(&merged)
    );
    assert!(text(&merged).contains("return 11;"));
    assert!(text(&merged).contains("return 22;"));
}

#[test]
fn edits_to_the_same_function_still_need_the_user() {
    let ours = BASE.replace("return 1;", "return 11;");
    let theirs = BASE.replace("return 1;", "return 111;");

    let merged = merge3_with_syntax(BASE, &ours, &theirs, Some("cpp"));
    assert!(
        merged.iter().any(Region::is_conflict),
        "{:?}",
        kinds(&merged)
    );
}

#[test]
fn a_settled_region_is_marked_so_it_can_be_looked_at() {
    // Adjoining edits: line-wise they are one region, and only the AST separates them.
    let base = "int a() { return 0; }\nint b() { return 0; }\n";
    let ours = "int a() { return 1; }\nint b() { return 0; }\n";
    let theirs = "int a() { return 0; }\nint b() { return 9; }\n";

    let merged = merge3_with_syntax(base, ours, theirs, Some("cpp"));
    assert!(merged.iter().any(|region| matches!(
        region,
        Region::Clean {
            origin: Origin::Syntactic,
            ..
        }
    )));
}

#[test]
fn without_a_language_nothing_is_settled_that_was_not_already() {
    let base = "int a() { return 0; }\nint b() { return 0; }\n";
    let ours = "int a() { return 1; }\nint b() { return 0; }\n";
    let theirs = "int a() { return 0; }\nint b() { return 9; }\n";

    assert_eq!(
        kinds(&merge3_with_syntax(base, ours, theirs, None)),
        kinds(&merge3(base, ours, theirs))
    );
}

#[test]
fn a_language_nobody_has_a_grammar_for_changes_nothing() {
    let base = "a\nb\n";
    let ours = "A\nb\n";
    let theirs = "a\nB\n";

    assert_eq!(
        kinds(&merge3_with_syntax(base, ours, theirs, Some("brainfuck"))),
        kinds(&merge3(base, ours, theirs))
    );
}

#[test]
fn typescript_is_understood_too() {
    let base = "function a() {\n  return 0;\n}\n\nfunction b() {\n  return 0;\n}\n";
    let ours = base.replace(
        "  return 0;\n}\n\nfunction b",
        "  return 1;\n}\n\nfunction b",
    );
    let theirs = base.replace("function b() {\n  return 0;", "function b() {\n  return 9;");

    let merged = merge3_with_syntax(base, &ours, &theirs, Some("typescript"));
    assert!(
        !merged.iter().any(Region::is_conflict),
        "{:?}",
        kinds(&merged)
    );
}

#[test]
fn a_file_that_does_not_parse_is_left_to_the_user() {
    // Half a function on each side: the tree is nonsense, so the nodes mean nothing.
    let base = "int a() { return 0;\n";
    let ours = "int a() { return 1;\n";
    let theirs = "int a() { return 9;\n";

    assert!(
        merge3_with_syntax(base, ours, theirs, Some("cpp"))
            .iter()
            .any(Region::is_conflict)
    );
}

#[test]
fn an_edit_that_spans_two_functions_is_not_settled() {
    // Ours replaces the whole body of the file in one stroke, so its edit belongs to no
    // single node and there is nothing to be certain about.
    let ours = "int only() {\n    return 7;\n}\n";
    let theirs = BASE.replace("return 2;", "return 22;");

    let merged = merge3_with_syntax(BASE, ours, &theirs, Some("cpp"));
    assert!(
        merged.iter().any(Region::is_conflict),
        "{:?}",
        kinds(&merged)
    );
}

#[test]
fn the_settled_text_keeps_the_order_of_the_file() {
    let base = "int a() { return 0; }\nint b() { return 0; }\n";
    let ours = "int a() { return 1; }\nint b() { return 0; }\n";
    let theirs = "int a() { return 0; }\nint b() { return 9; }\n";

    let merged = merge3_with_syntax(base, ours, theirs, Some("cpp"));
    let out = text(&merged);
    assert!(
        out.find("return 1;").unwrap() < out.find("return 9;").unwrap(),
        "{out}"
    );
}

#[test]
fn plain_merge3_never_settles_anything_syntactically() {
    let base = "int a() { return 0; }\nint b() { return 0; }\n";
    let ours = "int a() { return 1; }\nint b() { return 0; }\n";
    let theirs = "int a() { return 0; }\nint b() { return 9; }\n";

    assert!(!kinds(&merge3(base, ours, theirs)).contains(&"syntactic"));
}

// A `.tsx` file was parsed with the TypeScript grammar, which reads JSX as an error: the
// merge never settled anything in one.
#[test]
fn a_tsx_file_is_parsed_with_the_grammar_that_knows_jsx() {
    let base = "function a() {\n  return <b className=\"x\">0</b>;\n}\nfunction b() {\n  return <i>0</i>;\n}\n";
    let ours = base.replace("0</b>;\n}\n", "0</b>;\n} // a\n");
    let theirs = base.replace("function b() {", "function b(): JSX.Element {");
    assert!(
        merge3(base, &ours, &theirs).iter().any(Region::is_conflict),
        "the edits must be too close for a plain merge"
    );

    let grammar = diff_engine::merge_grammar_for_path("src/App.tsx");
    let merged = merge3_with_syntax(base, &ours, &theirs, grammar);

    assert!(
        !merged.iter().any(Region::is_conflict),
        "{:?}",
        kinds(&merged)
    );
}
