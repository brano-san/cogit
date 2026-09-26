/// The tree-sitter grammar the three-way merge parses a path with. `.tsx` has its own:
/// the TypeScript parser reads JSX as an error, and a tree with an error settles nothing.
#[must_use]
pub fn merge_grammar_for_path(path: &str) -> Option<&'static str> {
    language_for_path(path).and_then(|name| match name.as_str() {
        "typescript" => Some("typescript"),
        "tsx" => Some("tsx"),
        "cpp" => Some("cpp"),
        "c" => Some("c"),
        _ => None,
    })
}

/// Past this many lines the frontend does not parse a side whole. Must match
/// `MAX_HIGHLIGHT_LINES` in `frontend/src/lib/highlight.ts`.
pub const MAX_HIGHLIGHT_LINES: u32 = 5000;

/// The languages `frontend/src/lib/highlight.ts` has a Lezer parser for (`PARSERS`).
#[must_use]
pub fn highlighted(language: &str) -> bool {
    matches!(
        language,
        "c" | "cpp"
            | "css"
            | "html"
            | "javascript"
            | "json"
            | "python"
            | "rust"
            | "typescript"
            | "jsx"
            | "tsx"
    )
}

/// Lezer grammar name for a path. Highlighting itself is a frontend concern (INV-01). JSX
/// has names of its own: without the dialect a closing tag reads as a regular expression.
#[must_use]
pub fn language_for_path(path: &str) -> Option<String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let by_name = match name {
        "Makefile" | "makefile" | "GNUmakefile" => Some("makefile"),
        "Dockerfile" => Some("dockerfile"),
        "CMakeLists.txt" => Some("cmake"),
        _ => None,
    };
    if let Some(language) = by_name {
        return Some(language.to_owned());
    }

    let extension = name
        .rsplit_once('.')
        .map(|(_, ext)| ext)?
        .to_ascii_lowercase();
    let language = match extension.as_str() {
        "rs" => "rust",
        "ts" => "typescript",
        "tsx" => "tsx",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "py" | "pyi" => "python",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hxx" | "hh" => "cpp",
        "java" => "java",
        "go" => "go",
        "cs" => "csharp",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "kt" | "kts" => "kotlin",
        "sh" | "bash" | "zsh" => "shell",
        "sql" => "sql",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "xml" | "svg" => "xml",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" | "sass" => "sass",
        "md" | "markdown" => "markdown",
        "svelte" => "svelte",
        "vue" => "vue",
        _ => return None,
    };
    Some(language.to_owned())
}

#[cfg(test)]
mod tests {
    use super::language_for_path;

    #[test]
    fn maps_common_extensions() {
        assert_eq!(language_for_path("src/main.rs").as_deref(), Some("rust"));
        assert_eq!(language_for_path("a/b/app.tsx").as_deref(), Some("tsx"));
        assert_eq!(language_for_path("view.jsx").as_deref(), Some("jsx"));
        assert_eq!(language_for_path("app.ts").as_deref(), Some("typescript"));
        assert_eq!(language_for_path("engine.hpp").as_deref(), Some("cpp"));
    }

    #[test]
    fn recognises_files_that_have_no_extension() {
        assert_eq!(
            language_for_path("build/Makefile").as_deref(),
            Some("makefile")
        );
        assert_eq!(
            language_for_path("Dockerfile").as_deref(),
            Some("dockerfile")
        );
    }

    #[test]
    fn ignores_the_case_of_the_extension() {
        assert_eq!(language_for_path("README.MD").as_deref(), Some("markdown"));
    }

    #[test]
    fn a_dotfile_is_not_read_as_an_extension() {
        assert_eq!(language_for_path(".gitignore"), None);
    }

    #[test]
    fn an_unknown_extension_has_no_grammar() {
        assert_eq!(language_for_path("data.bin"), None);
        assert_eq!(language_for_path("LICENSE"), None);
    }
}
