use serde::{Deserialize, Serialize};
use spellbook::Dictionary;
use std::sync::OnceLock;

const MAX_SUGGESTIONS: usize = 8;

pub const NOTICES: &str = concat!(
    "Spelling dictionaries\n\nen_US (LibreOffice, from SCOWL)\n\n",
    include_str!("../dictionaries/README_en_US.txt"),
    "\n\nru_RU (LibreOffice)\n\n",
    include_str!("../dictionaries/README_ru_RU.txt"),
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub enum SpellLanguage {
    #[serde(rename = "en_US")]
    EnUs,
    #[serde(rename = "ru_RU")]
    RuRu,
}

impl SpellLanguage {
    fn covers(self, word: &str) -> bool {
        let letters = word.chars().filter(|ch| ch.is_alphabetic());
        match self {
            Self::EnUs => {
                letters.clone().count() > 0
                    && letters.into_iter().all(|ch| ch.is_ascii_alphabetic())
            }
            Self::RuRu => letters.clone().count() > 0 && letters.into_iter().all(is_cyrillic),
        }
    }

    fn dictionary(self) -> Option<&'static Dictionary> {
        static EN_US: OnceLock<Option<Dictionary>> = OnceLock::new();
        static RU_RU: OnceLock<Option<Dictionary>> = OnceLock::new();
        let (cell, aff, dic, extra) = match self {
            Self::EnUs => (
                &EN_US,
                include_str!("../dictionaries/en_US.aff"),
                include_str!("../dictionaries/en_US.dic"),
                include_str!("../dictionaries/en_US_dev.dic"),
            ),
            Self::RuRu => (
                &RU_RU,
                include_str!("../dictionaries/ru_RU.aff"),
                include_str!("../dictionaries/ru_RU.dic"),
                include_str!("../dictionaries/ru_RU_dev.dic"),
            ),
        };
        cell.get_or_init(|| {
            let started = std::time::Instant::now();
            match Dictionary::new(aff, dic) {
                Ok(mut dictionary) => {
                    for entry in extra.lines().filter(|line| !line.is_empty()) {
                        if let Err(err) = dictionary.add(entry) {
                            tracing::error!(error = ?err, entry, context = "spelling supplement");
                        }
                    }
                    tracing::info!(language = ?self, elapsed = ?started.elapsed(), "spelling dictionary loaded");
                    Some(dictionary)
                }
                Err(err) => {
                    tracing::error!(error = ?err, language = ?self, context = "spelling dictionary");
                    None
                }
            }
        })
        .as_ref()
    }
}

fn is_cyrillic(ch: char) -> bool {
    matches!(ch, '\u{0400}'..='\u{04FF}')
}

fn covering(languages: &[SpellLanguage], word: &str) -> impl Iterator<Item = &'static Dictionary> {
    languages
        .iter()
        .filter(|language| language.covers(word))
        .filter_map(|language| language.dictionary())
        .collect::<Vec<_>>()
        .into_iter()
}

#[must_use]
pub fn misspelled(languages: &[SpellLanguage], words: &[String]) -> Vec<String> {
    words
        .iter()
        .filter(|word| {
            let mut dictionaries = covering(languages, word).peekable();
            dictionaries.peek().is_some() && !dictionaries.any(|dictionary| dictionary.check(word))
        })
        .cloned()
        .collect()
}

#[must_use]
pub fn suggestions(languages: &[SpellLanguage], word: &str) -> Vec<String> {
    let mut out = Vec::new();
    for dictionary in covering(languages, word) {
        let mut found = Vec::new();
        dictionary.suggest(word, &mut found);
        for suggestion in found {
            if !out.contains(&suggestion) {
                out.push(suggestion);
            }
        }
    }
    out.truncate(MAX_SUGGESTIONS);
    out
}
