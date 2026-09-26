//! The code-block languages of Vue `CODE_BLOCK_LANGUAGES`/`SHIKI_LANGUAGES`
//! (`TiptapEditor.vue`), mapped onto gpui-component's `LanguageRegistry`.
//! Twenty-five resolve through gpui-component features; six (`ini`, `less`,
//! `objective-c`, `perl`, `r`, `xml`) come from direct grammar crates
//! registered here.
//!
//! Fence aliases normalize like Vue `languageAliasToValue` (`js`→`javascript`,
//! `objc`→`objective-c`, …). A language whose grammar fails to register still
//! shows in the picker and renders as plain text — the issue's GAP fallback.

use gpui::SharedString;
use gpui_component::highlighter::{GrammarConfig, LanguageRegistry};
use std::sync::OnceLock;

/// One pickable language.
#[derive(Clone, Copy, Debug)]
pub struct Lang {
    /// Canonical fence token (Vue `value`).
    pub name: &'static str,
    /// Picker label (Vue `label`).
    pub label: &'static str,
    /// Registry key — differs only where gpui-component canonicalizes
    /// (`makefile` → `make`).
    pub registry: &'static str,
    /// Vue `aliases` — normalize to `name` (and match picker search).
    pub aliases: &'static [&'static str],
}

/// Vue `CODE_BLOCK_LANGUAGES`, in Vue order — "Plain text" (`""`) first.
#[rustfmt::skip]
pub const LANGUAGES: &[Lang] = &[
    Lang { name: "", label: "Plain text", registry: "plaintext", aliases: &["text", "plain", "plain text"] },
    Lang { name: "bash", label: "Bash / Shell", registry: "bash", aliases: &["sh", "shell", "zsh"] },
    Lang { name: "c", label: "C", registry: "c", aliases: &[] },
    Lang { name: "cpp", label: "C++", registry: "cpp", aliases: &["c++", "cc", "cxx"] },
    Lang { name: "csharp", label: "C Sharp", registry: "csharp", aliases: &["cs", "c#"] },
    Lang { name: "css", label: "CSS", registry: "css", aliases: &[] },
    Lang { name: "diff", label: "Diff", registry: "diff", aliases: &["patch"] },
    Lang { name: "go", label: "Go", registry: "go", aliases: &["golang"] },
    Lang { name: "graphql", label: "GraphQL", registry: "graphql", aliases: &["gql"] },
    Lang { name: "html", label: "HTML", registry: "html", aliases: &[] },
    Lang { name: "ini", label: "INI", registry: "ini", aliases: &["conf", "cfg"] },
    Lang { name: "java", label: "Java", registry: "java", aliases: &[] },
    Lang { name: "javascript", label: "JavaScript", registry: "javascript", aliases: &["js", "jsx"] },
    Lang { name: "json", label: "JSON", registry: "json", aliases: &[] },
    Lang { name: "kotlin", label: "Kotlin", registry: "kotlin", aliases: &["kt", "kts"] },
    Lang { name: "less", label: "Less", registry: "less", aliases: &[] },
    Lang { name: "lua", label: "Lua", registry: "lua", aliases: &[] },
    Lang { name: "makefile", label: "Makefile", registry: "make", aliases: &["make"] },
    Lang { name: "markdown", label: "Markdown", registry: "markdown", aliases: &["md"] },
    Lang { name: "objective-c", label: "Objective-C", registry: "objective-c", aliases: &["objc", "objectivec"] },
    Lang { name: "perl", label: "Perl", registry: "perl", aliases: &["pl"] },
    Lang { name: "php", label: "PHP", registry: "php", aliases: &[] },
    Lang { name: "python", label: "Python", registry: "python", aliases: &["py"] },
    Lang { name: "r", label: "R", registry: "r", aliases: &[] },
    Lang { name: "ruby", label: "Ruby", registry: "ruby", aliases: &["rb"] },
    Lang { name: "rust", label: "Rust", registry: "rust", aliases: &["rs"] },
    Lang { name: "scss", label: "SCSS", registry: "scss", aliases: &[] },
    Lang { name: "sql", label: "SQL", registry: "sql", aliases: &[] },
    Lang { name: "swift", label: "Swift", registry: "swift", aliases: &[] },
    Lang { name: "typescript", label: "TypeScript", registry: "typescript", aliases: &["ts", "tsx"] },
    Lang { name: "xml", label: "XML", registry: "xml", aliases: &[] },
    Lang { name: "yaml", label: "YAML", registry: "yaml", aliases: &["yml"] },
];

/// Register the six direct grammars; idempotent.
pub fn init_languages() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        let reg = LanguageRegistry::singleton();
        for &(name, lang, hl, inj, locals) in EXTRA {
            reg.register(
                name,
                &GrammarConfig::new(name, (lang)(), vec![], hl, inj, locals),
            );
        }
    });
}

type LangFn = fn() -> tree_sitter::Language;

/// `(registry name, grammar getter, highlights query, injections, locals)` —
/// the six languages gpui-component does not ship.
static EXTRA: &[(&str, LangFn, &str, &str, &str)] = &[
    (
        "ini",
        || tree_sitter_ini::LANGUAGE.into(),
        tree_sitter_ini::HIGHLIGHTS_QUERY,
        "",
        "",
    ),
    (
        "less",
        tree_sitter_less::language,
        tree_sitter_less::HIGHLIGHTS_QUERY,
        "",
        "",
    ),
    (
        "objective-c",
        || tree_sitter_objc::LANGUAGE.into(),
        tree_sitter_objc::HIGHLIGHTS_QUERY,
        tree_sitter_objc::INJECTIONS_QUERY,
        tree_sitter_objc::LOCALS_QUERY,
    ),
    // tree-sitter-perl 1.1 ships no bundled queries → registers for parsing,
    // highlights stay plain (documented GAP in PARITY.md).
    ("perl", || tree_sitter_perl::LANGUAGE.into(), "", "", ""),
    (
        "r",
        || tree_sitter_r::LANGUAGE.into(),
        tree_sitter_r::HIGHLIGHTS_QUERY,
        "",
        tree_sitter_r::LOCALS_QUERY,
    ),
    (
        "xml",
        || tree_sitter_xml::LANGUAGE_XML.into(),
        tree_sitter_xml::XML_HIGHLIGHT_QUERY,
        "",
        "",
    ),
];

/// Vue `languageAliasToValue` — fence token/alias → canonical `Lang`.
pub fn lookup(fence_lang: &str) -> Option<&'static Lang> {
    let l = fence_lang.trim().to_lowercase();
    LANGUAGES
        .iter()
        .find(|lang| lang.name == l || lang.aliases.iter().any(|a| *a == l))
}

/// Picker filter: case-insensitive substring over name, label and aliases.
pub fn filter_languages(query: &str) -> Vec<&'static Lang> {
    let q = query.trim().to_lowercase();
    LANGUAGES
        .iter()
        .filter(|l| {
            q.is_empty()
                || l.name.contains(q.as_str())
                || l.label.to_lowercase().contains(q.as_str())
                || l.aliases.iter().any(|a| a.contains(q.as_str()))
        })
        .collect()
}

/// Fence token → registry name (None = unknown; still degrades to plain).
pub fn registry_name(fence_lang: &str) -> Option<&'static str> {
    lookup(fence_lang).map(|l| l.registry)
}

/// `true` when a grammar is available (registered or built-in).
pub fn has_grammar(fence_lang: &str) -> bool {
    init_languages();
    let name = match registry_name(fence_lang) {
        Some(n) => SharedString::from(n),
        None => SharedString::from(fence_lang.trim().to_lowercase()),
    };
    LanguageRegistry::singleton()
        .language(&name)
        .is_some_and(|cfg| cfg.has_grammar())
}
