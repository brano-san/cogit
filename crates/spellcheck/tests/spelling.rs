use spellcheck::{SpellLanguage, misspelled, suggestions};

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|word| (*word).to_owned()).collect()
}

#[test]
fn english_words_pass_and_typos_are_reported() {
    let found = misspelled(
        &[SpellLanguage::EnUs],
        &words(&["Fix", "the", "recieve", "handler", "teh"]),
    );
    assert_eq!(found, words(&["recieve", "teh"]));
}

#[test]
fn russian_words_pass_and_typos_are_reported() {
    let found = misspelled(
        &[SpellLanguage::RuRu],
        &words(&["Исправлено", "открытие", "репозитория", "превет"]),
    );
    assert_eq!(found, words(&["превет"]));
}

#[test]
fn russian_development_words_pass_in_their_forms() {
    let forms = words(&[
        "коммит",
        "коммита",
        "коммиты",
        "коммитов",
        "репозиторий",
        "репозитория",
        "репозиториев",
        "сабмодули",
        "ребейза",
        "хэша",
        "закоммитил",
        "закоммитить",
        "запушил",
        "смержил",
    ]);
    assert!(
        misspelled(&[SpellLanguage::RuRu], &forms).is_empty(),
        "{:?}",
        misspelled(&[SpellLanguage::RuRu], &forms)
    );
}

#[test]
fn a_word_in_a_script_no_chosen_dictionary_covers_is_left_alone() {
    assert!(misspelled(&[SpellLanguage::EnUs], &words(&["превет"])).is_empty());
    assert!(misspelled(&[SpellLanguage::RuRu], &words(&["teh"])).is_empty());
}

#[test]
fn both_dictionaries_check_a_mixed_message() {
    let found = misspelled(
        &[SpellLanguage::EnUs, SpellLanguage::RuRu],
        &words(&["Fix", "teh", "открытие", "превет"]),
    );
    assert_eq!(found, words(&["teh", "превет"]));
}

#[test]
fn no_dictionary_reports_nothing() {
    assert!(misspelled(&[], &words(&["teh", "превет"])).is_empty());
}

#[test]
fn suggestions_come_from_the_dictionary_of_the_word() {
    assert!(
        suggestions(&[SpellLanguage::EnUs, SpellLanguage::RuRu], "recieve")
            .contains(&"receive".to_owned())
    );
    assert!(
        suggestions(&[SpellLanguage::EnUs, SpellLanguage::RuRu], "превет")
            .contains(&"привет".to_owned())
    );
    assert!(suggestions(&[SpellLanguage::EnUs], "превет").is_empty());
}

#[test]
fn english_development_words_pass() {
    let list = words(&[
        "refactor",
        "refactors",
        "refactored",
        "refactoring",
        "rebase",
        "rebased",
        "rebases",
        "rebasing",
        "submodule",
        "submodules",
        "config",
        "configs",
        "async",
        "backend",
        "backends",
        "frontend",
        "frontends",
        "repo",
        "repos",
        "changelog",
        "linter",
        "timeout",
        "timeouts",
        "hotfix",
        "hotfixes",
        "webview",
        "toolbar",
        "dropdown",
        "tooltip",
        "tooltips",
        "checkbox",
        "checkboxes",
        "namespace",
        "runtime",
        "codebase",
        "middleware",
        "localhost",
        "unstage",
        "unstaged",
        "unstaging",
        "workflow",
        "workflows",
        "dedupe",
        "enum",
        "enums",
        "struct",
        "structs",
        "bool",
        "params",
        "args",
        "auth",
        "init",
        "debounce",
        "debounced",
        "serialize",
        "deserialize",
        "tokenize",
        "tokenizer",
        "parser",
        "lookup",
        "bugfix",
        "cleanup",
        "renderer",
        "username",
        "login",
        "logout",
        "upstream",
        "rollback",
        "squash",
        "squashed",
        "stash",
        "stashed",
        "amend",
        "amended",
        "fixup",
        "commit",
        "commits",
        "committed",
        "metadata",
        "plugin",
        "plugins",
        "online",
        "offline",
        "inline",
        "endpoint",
        "endpoints",
        "Fixes",
        "Adds",
        "Removes",
        "Bumps",
        "it's",
        "doesn't",
        "can't",
    ]);
    assert!(
        misspelled(&[SpellLanguage::EnUs], &list).is_empty(),
        "{:?}",
        misspelled(&[SpellLanguage::EnUs], &list)
    );
}

#[test]
fn a_word_with_combining_marks_is_checked_as_composed() {
    let decomposed = "\u{0435}\u{0449}\u{0435}\u{0308}".to_owned();
    let short = "\u{0438}\u{0306}\u{043e}\u{0434}".to_owned();
    assert!(misspelled(&[SpellLanguage::RuRu], &[decomposed, short]).is_empty());
}
