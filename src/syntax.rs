use crate::framebuffer::{Framebuffer, IndexedColor};
use crate::oklab::StraightRgba;
use std::sync::Arc;
use tree_sitter::{Parser, Tree};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

// Embedded highlight queries for languages that don't export them in their crates

const KOTLIN_HIGHLIGHTS: &str = include_str!("../queries/kotlin_highlights.scm");
const SQL_HIGHLIGHTS: &str = include_str!("../queries/sql_highlights.scm");
const DOCKERFILE_HIGHLIGHTS: &str = include_str!("../queries/dockerfile_highlights.scm");
const MARKDOWN_HIGHLIGHTS: &str = include_str!("../queries/markdown_highlights.scm");

// External C functions for languages with version incompatibility
// These crates use older tree-sitter versions, so we call the C functions directly
unsafe extern "C" {
    fn tree_sitter_kotlin() -> tree_sitter::Language;
    fn tree_sitter_markdown() -> tree_sitter::Language;
    fn tree_sitter_sql() -> tree_sitter::Language;
    fn tree_sitter_dockerfile() -> tree_sitter::Language;
}

// Dummy references to ensure the crates' C code gets linked
#[cfg(feature = "syntax-kotlin")]
const _KOTLIN_LINK: &str = tree_sitter_kotlin::NODE_TYPES;
#[cfg(feature = "syntax-markdown")]
const _MARKDOWN_LINK: &str = tree_sitter_markdown::NODE_TYPES;
#[cfg(feature = "syntax-sql")]
const _SQL_LINK: &str = tree_sitter_sql::NODE_TYPES;
#[cfg(feature = "syntax-dockerfile")]
const _DOCKERFILE_LINK: &str = tree_sitter_dockerfile::NODE_TYPES;

/// Represents a highlighted span in the source code
#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub start_byte: usize,
    pub end_byte: usize,
    pub color: StraightRgba,
}

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    // Programming languages
    Rust,
    Python,
    JavaScript,
    TypeScript,
    C,
    Cpp,
    Swift,
    Go,
    Java,
    Ruby,
    Php,
    Kotlin,
    Scala,
    Haskell,
    Elixir,
    Zig,
    // Web languages
    Html,
    Css,
    // Markup and data formats
    Markdown,
    Json,
    Yaml,
    Toml,
    Xml,
    // Shell and config
    Bash,
    Sql,
    Dockerfile,
}

impl Language {
    /// Detect language from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            // Rust
            "rs" => Some(Language::Rust),
            // Python
            "py" | "pyw" | "pyi" => Some(Language::Python),
            // JavaScript/TypeScript
            "js" | "mjs" | "cjs" | "jsx" => Some(Language::JavaScript),
            "ts" | "mts" | "cts" | "tsx" => Some(Language::TypeScript),
            // C/C++
            "c" | "h" => Some(Language::C),
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" | "c++" => Some(Language::Cpp),
            // Swift
            "swift" => Some(Language::Swift),
            // Go
            "go" => Some(Language::Go),
            // Java
            "java" => Some(Language::Java),
            // Ruby
            "rb" | "rake" | "gemspec" => Some(Language::Ruby),
            // PHP
            "php" | "phtml" | "php3" | "php4" | "php5" | "php7" | "phps" => Some(Language::Php),
            // Kotlin
            "kt" | "kts" => Some(Language::Kotlin),
            // Scala
            "scala" | "sc" => Some(Language::Scala),
            // Haskell
            "hs" | "lhs" => Some(Language::Haskell),
            // Elixir
            "ex" | "exs" => Some(Language::Elixir),
            // Zig
            "zig" => Some(Language::Zig),
            // Web languages
            "html" | "htm" => Some(Language::Html),
            "css" => Some(Language::Css),
            // Markup and data
            "md" | "markdown" | "mkd" | "mkdn" => Some(Language::Markdown),
            "json" | "jsonc" => Some(Language::Json),
            "yaml" | "yml" => Some(Language::Yaml),
            "toml" => Some(Language::Toml),
            "xml" | "xsl" | "xsd" | "svg" => Some(Language::Xml),
            // Shell
            "sh" | "bash" | "zsh" => Some(Language::Bash),
            // SQL
            "sql" | "mysql" | "pgsql" => Some(Language::Sql),
            _ => None,
        }
    }

    /// Detect language from filename (for special files without extensions)
    pub fn from_filename(filename: &str) -> Option<Self> {
        match filename.to_lowercase().as_str() {
            "dockerfile" | "containerfile" => Some(Language::Dockerfile),
            "makefile" | "gnumakefile" => Some(Language::Bash), // Makefile uses shell syntax often
            "rakefile" | "gemfile" => Some(Language::Ruby),
            _ => None,
        }
    }

}

/// Manages syntax highlighting for a document
pub struct SyntaxHighlighter {
    language: Language,
    parser: Parser,
    tree: Option<Tree>,
    config: Arc<HighlightConfiguration>,
    pub buffer_generation: u32,
    highlight_names: Vec<String>,
    source_cache: Vec<u8>,
    // Cache of all highlight spans for the entire file
    highlight_cache: Vec<HighlightSpan>,
    highlight_cache_generation: u32,
}

impl SyntaxHighlighter {
    /// Create a new syntax highlighter for the given language
    pub fn new(language: Language) -> Result<Self, String> {
        let mut parser = Parser::new();
        let (tree_sitter_lang, highlight_query, injection_query, locals_query) =
            get_language_config(language)?;

        parser
            .set_language(&tree_sitter_lang)
            .map_err(|e| format!("Failed to set parser language: {}", e))?;

        let mut config = HighlightConfiguration::new(
            tree_sitter_lang,
            "source", // scope name
            highlight_query,
            injection_query.unwrap_or(""),
            locals_query.unwrap_or(""),
        )
        .map_err(|e| format!("Failed to create highlight configuration: {}", e))?;

        // Standard highlight names (TextMate-compatible)
        let highlight_names = vec![
            "attribute",
            "comment",
            "constant",
            "constant.builtin",
            "constructor",
            "embedded",
            "function",
            "function.builtin",
            "function.method",
            "keyword",
            "number",
            "operator",
            "property",
            "punctuation",
            "punctuation.bracket",
            "punctuation.delimiter",
            "string",
            "string.special",
            "tag",
            "type",
            "type.builtin",
            "variable",
            "variable.builtin",
            "variable.parameter",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

        config.configure(&highlight_names);

        Ok(Self {
            language,
            parser,
            tree: None,
            config: Arc::new(config),
            buffer_generation: 0,
            highlight_names,
            source_cache: Vec::new(),
            highlight_cache: Vec::new(),
            highlight_cache_generation: 0,
        })
    }

    /// Update the syntax tree if the buffer has changed
    /// This caches the source buffer to avoid re-collecting it on every render
    pub fn update(&mut self, source: Vec<u8>, buffer_generation: u32) {
        if self.buffer_generation == buffer_generation && self.tree.is_some() {
            return; // Already up to date
        }

        self.tree = self.parser.parse(&source, None);
        self.source_cache = source;
        self.buffer_generation = buffer_generation;
    }

    /// Get a reference to the cached source buffer
    pub fn cached_source(&self) -> &[u8] {
        &self.source_cache
    }

    /// Ensure highlights are cached for the current buffer generation
    fn ensure_highlights_cached(&mut self, fb: &Framebuffer) {
        // Check if cache is already up to date
        if self.highlight_cache_generation == self.buffer_generation && !self.highlight_cache.is_empty() {
            return;
        }

        // Clear old cache
        self.highlight_cache.clear();

        if self.tree.is_none() || self.source_cache.is_empty() {
            return;
        }

        // Compute highlights for entire file
        let mut highlighter = Highlighter::new();
        let highlight_iter = match highlighter.highlight(&self.config, &self.source_cache, None, |_| None) {
            Ok(iter) => iter,
            Err(_) => return,
        };

        let mut current_highlight: Option<usize> = None;

        for event in highlight_iter {
            match event {
                Ok(HighlightEvent::Source { start, end }) => {
                    if let Some(highlight_idx) = current_highlight {
                        let color = get_highlight_color(
                            self.highlight_names
                                .get(highlight_idx)
                                .map(|s| s.as_str())
                                .unwrap_or(""),
                            fb,
                        );
                        self.highlight_cache.push(HighlightSpan {
                            start_byte: start,
                            end_byte: end,
                            color,
                        });
                    }
                }
                Ok(HighlightEvent::HighlightStart(idx)) => {
                    current_highlight = Some(idx.0);
                }
                Ok(HighlightEvent::HighlightEnd) => {
                    current_highlight = None;
                }
                Err(_) => break,
            }
        }

        self.highlight_cache_generation = self.buffer_generation;
    }

    /// Get highlight spans for a byte range
    pub fn get_highlights(
        &mut self,
        start_byte: usize,
        end_byte: usize,
        fb: &Framebuffer,
    ) -> Vec<HighlightSpan> {
        self.ensure_highlights_cached(fb);

        // Filter cached highlights to the requested range
        self.highlight_cache
            .iter()
            .filter(|h| h.end_byte > start_byte && h.start_byte < end_byte)
            .map(|h| HighlightSpan {
                start_byte: h.start_byte.max(start_byte),
                end_byte: h.end_byte.min(end_byte),
                color: h.color,
            })
            .collect()
    }

    pub fn language(&self) -> Language {
        self.language
    }
}

/// Get language configuration (parser, queries)
fn get_language_config(
    language: Language,
) -> Result<(tree_sitter::Language, &'static str, Option<&'static str>, Option<&'static str>), String>
{
    match language {
        #[cfg(feature = "syntax-rust")]
        Language::Rust => Ok((
            tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            Some(tree_sitter_rust::INJECTIONS_QUERY),
            None,
        )),
        #[cfg(feature = "syntax-python")]
        Language::Python => Ok((
            tree_sitter_python::LANGUAGE.into(),
            tree_sitter_python::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-javascript")]
        Language::JavaScript => Ok((
            tree_sitter_javascript::LANGUAGE.into(),
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            Some(tree_sitter_javascript::INJECTIONS_QUERY),
            Some(tree_sitter_javascript::LOCALS_QUERY),
        )),
        #[cfg(feature = "syntax-c")]
        Language::C => Ok((
            tree_sitter_c::LANGUAGE.into(),
            tree_sitter_c::HIGHLIGHT_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-cpp")]
        Language::Cpp => Ok((
            tree_sitter_cpp::LANGUAGE.into(),
            tree_sitter_cpp::HIGHLIGHT_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-swift")]
        Language::Swift => Ok((
            tree_sitter_swift::LANGUAGE.into(),
            tree_sitter_swift::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-json")]
        Language::Json => Ok((
            tree_sitter_json::LANGUAGE.into(),
            tree_sitter_json::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-bash")]
        Language::Bash => Ok((
            tree_sitter_bash::LANGUAGE.into(),
            tree_sitter_bash::HIGHLIGHT_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-typescript")]
        Language::TypeScript => Ok((
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            None,
            Some(tree_sitter_typescript::LOCALS_QUERY),
        )),
        #[cfg(feature = "syntax-go")]
        Language::Go => Ok((
            tree_sitter_go::LANGUAGE.into(),
            tree_sitter_go::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-java")]
        Language::Java => Ok((
            tree_sitter_java::LANGUAGE.into(),
            tree_sitter_java::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-ruby")]
        Language::Ruby => Ok((
            tree_sitter_ruby::LANGUAGE.into(),
            tree_sitter_ruby::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-php")]
        Language::Php => Ok((
            tree_sitter_php::LANGUAGE_PHP.into(),
            tree_sitter_php::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-kotlin")]
        Language::Kotlin => Ok((
            unsafe { tree_sitter_kotlin() },
            KOTLIN_HIGHLIGHTS,
            None,
            None,
        )),
        #[cfg(feature = "syntax-scala")]
        Language::Scala => Ok((
            tree_sitter_scala::LANGUAGE.into(),
            tree_sitter_scala::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-haskell")]
        Language::Haskell => Ok((
            tree_sitter_haskell::LANGUAGE.into(),
            tree_sitter_haskell::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-elixir")]
        Language::Elixir => Ok((
            tree_sitter_elixir::LANGUAGE.into(),
            tree_sitter_elixir::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-zig")]
        Language::Zig => Ok((
            tree_sitter_zig::LANGUAGE.into(),
            tree_sitter_zig::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-html")]
        Language::Html => Ok((
            tree_sitter_html::LANGUAGE.into(),
            tree_sitter_html::HIGHLIGHTS_QUERY,
            Some(tree_sitter_html::INJECTIONS_QUERY),
            None,
        )),
        #[cfg(feature = "syntax-css")]
        Language::Css => Ok((
            tree_sitter_css::LANGUAGE.into(),
            tree_sitter_css::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-markdown")]
        Language::Markdown => Ok((
            unsafe { tree_sitter_markdown() },
            MARKDOWN_HIGHLIGHTS,
            None,
            None,
        )),
        #[cfg(feature = "syntax-yaml")]
        Language::Yaml => Ok((
            tree_sitter_yaml::LANGUAGE.into(),
            tree_sitter_yaml::HIGHLIGHTS_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-toml")]
        Language::Toml => Ok((
            tree_sitter_toml::language(),
            tree_sitter_toml::HIGHLIGHT_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-xml")]
        Language::Xml => Ok((
            tree_sitter_xml::LANGUAGE_XML.into(),
            tree_sitter_xml::XML_HIGHLIGHT_QUERY,
            None,
            None,
        )),
        #[cfg(feature = "syntax-sql")]
        Language::Sql => Ok((
            unsafe { tree_sitter_sql() },
            SQL_HIGHLIGHTS,
            None,
            None,
        )),
        #[cfg(feature = "syntax-dockerfile")]
        Language::Dockerfile => Ok((
            unsafe { tree_sitter_dockerfile() },
            DOCKERFILE_HIGHLIGHTS,
            None,
            None,
        )),
        // Disabled languages (version incompatibility)
        Language::Toml => Err(format!(
            "Language {:?} is temporarily disabled (tree-sitter version incompatibility).",
            language
        )),
        #[cfg(not(any(
            feature = "syntax-rust",
            feature = "syntax-python",
            feature = "syntax-javascript",
            feature = "syntax-typescript",
            feature = "syntax-c",
            feature = "syntax-cpp",
            feature = "syntax-swift",
            feature = "syntax-go",
            feature = "syntax-java",
            feature = "syntax-ruby",
            feature = "syntax-php",
            feature = "syntax-kotlin",
            feature = "syntax-scala",
            feature = "syntax-haskell",
            feature = "syntax-elixir",
            feature = "syntax-zig",
            feature = "syntax-html",
            feature = "syntax-css",
            feature = "syntax-markdown",
            feature = "syntax-json",
            feature = "syntax-yaml",
            feature = "syntax-toml",
            feature = "syntax-xml",
            feature = "syntax-bash",
            feature = "syntax-sql",
            feature = "syntax-dockerfile",
        )))]
        _ => Err(format!(
            "Language {:?} is not enabled. Enable the corresponding feature flag.",
            language
        )),
    }
}

/// Map tree-sitter highlight scope names to terminal colors
fn get_highlight_color(scope: &str, fb: &Framebuffer) -> StraightRgba {
    match scope {
        "comment" => fb.indexed(IndexedColor::BrightBlack),
        "keyword" => fb.indexed(IndexedColor::Magenta),
        "function" | "function.method" | "function.builtin" => fb.indexed(IndexedColor::Blue),
        "string" | "string.special" => fb.indexed(IndexedColor::Green),
        "type" | "type.builtin" => fb.indexed(IndexedColor::Cyan),
        "constant" | "constant.builtin" | "number" => fb.indexed(IndexedColor::Yellow),
        "variable.parameter" => fb.indexed(IndexedColor::BrightCyan),
        "operator" => fb.indexed(IndexedColor::BrightWhite),
        "property" => fb.indexed(IndexedColor::BrightBlue),
        "attribute" => fb.indexed(IndexedColor::BrightYellow),
        "constructor" => fb.indexed(IndexedColor::BrightMagenta),
        "tag" => fb.indexed(IndexedColor::Red),
        "punctuation" | "punctuation.bracket" | "punctuation.delimiter" => {
            fb.indexed(IndexedColor::Foreground)
        }
        "variable" | "variable.builtin" => fb.indexed(IndexedColor::Foreground),
        _ => fb.indexed(IndexedColor::Foreground),
    }
}
