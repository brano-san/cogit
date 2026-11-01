use spellcheck::SpellLanguage;

use super::blocking_or_default;

#[tauri::command]
#[specta::specta]
pub async fn spelling_misspelled(languages: Vec<SpellLanguage>, words: Vec<String>) -> Vec<String> {
    blocking_or_default("spelling_misspelled", move || {
        spellcheck::misspelled(&languages, &words)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn spelling_suggestions(languages: Vec<SpellLanguage>, word: String) -> Vec<String> {
    blocking_or_default("spelling_suggestions", move || {
        spellcheck::suggestions(&languages, &word)
    })
    .await
}
