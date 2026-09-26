//! The 31 languages of Vue `SHIKI_LANGUAGES` (`TiptapEditor.vue`), mapped onto
//! gpui-component's `LanguageRegistry`. Twenty-five resolve through
//! gpui-component features; six (`ini`, `less`, `objective-c`, `perl`, `r`,
//! `xml`) come from direct grammar crates registered here.
//!
//! A language whose grammar fails to register still shows in the picker and
//! renders as plain text — the GAP fallback the issue calls for.

use gpui::SharedString;
use gpui_component::highlighter::{GrammarConfig, LanguageRegistry};
use std::sync::OnceLock;

/// One pickable language.
#[derive(Clone, Copy, Debug)]
pub struct Lang {
    /// Fence-info / picker key (the Vue `SHIKI_LANGUAGES` token).
    pub name: &'static str,
    /// Registry key — differs only where gpui-component canonicalizes
    /// (`makefile` → `make`, `objective-c` → `objc`).
    pub registry: &'static str,
}

/// The Vue `SHIKI_LANGUAGES` table, in Vue order.
pub const LANGUAGES: &[Lang] = &[
    Lang {
        name: "bash",
        registry: "bash",
    },
    Lang {
        name: "c",
        registry: "c",
    },
    Lang {
        name: "cpp",
        registry: "cpp",
    },
    Lang {
        name: "csharp",
        registry: "csharp",
    },
    Lang {
        name: "css",
        registry: "css",
    },
    Lang {
        name: "diff",
        registry: "diff",
    },
    Lang {
        name: "go",
        registry: "go",
    },
    Lang {
        name: "graphql",
        registry: "graphql",
    },
    Lang {
        name: "html",
        registry: "html",
    },
    Lang {
        name: "ini",
        registry: "ini",
    },
    Lang {
        name: "java",
        registry: "java",
    },
    Lang {
        name: "javascript",
        registry: "javascript",
    },
    Lang {
        name: "json",
        registry: "json",
    },
    Lang {
        name: "kotlin",
        registry: "kotlin",
    },
    Lang {
        name: "less",
        registry: "less",
    },
    Lang {
        name: "lua",
        registry: "lua",
    },
    Lang {
        name: "makefile",
        registry: "make",
    },
    Lang {
        name: "markdown",
        registry: "markdown",
    },
    Lang {
        name: "objective-c",
        registry: "objective-c",
    },
    Lang {
        name: "perl",
        registry: "perl",
    },
    Lang {
        name: "php",
        registry: "php",
    },
    Lang {
        name: "python",
        registry: "python",
    },
    Lang {
        name: "r",
        registry: "r",
    },
    Lang {
        name: "ruby",
        registry: "ruby",
    },
    Lang {
        name: "rust",
        registry: "rust",
    },
    Lang {
        name: "scss",
        registry: "scss",
    },
    Lang {
        name: "sql",
        registry: "sql",
    },
    Lang {
        name: "swift",
        registry: "swift",
    },
    Lang {
        name: "typescript",
        registry: "typescript",
    },
    Lang {
        name: "xml",
        registry: "xml",
    },
    Lang {
        name: "yaml",
        registry: "yaml",
    },
];

/// Register the six direct grammars; idempotent. Returns names that ended up
/// without a grammar (for GAP reporting/tests).
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

/// Picker/filter lookup: case-insensitive prefix-substring over the Vue list.
pub fn filter_languages(query: &str) -> Vec<&'static Lang> {
    let q = query.trim().to_lowercase();
    LANGUAGES
        .iter()
        .filter(|l| q.is_empty() || l.name.contains(q.as_str()))
        .collect()
}

/// Vue name → registry name (None = not in the 31; still registered on demand
/// so unknown fence langs degrade to plain).
pub fn registry_name(fence_lang: &str) -> Option<&'static str> {
    let l = fence_lang.trim().to_lowercase();
    LANGUAGES
        .iter()
        .find(|lang| lang.name == l)
        .map(|l| l.registry)
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
