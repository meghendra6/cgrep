// SPDX-License-Identifier: MIT OR Apache-2.0

//! Symbol extraction from AST using tree-sitter node traversal

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tree_sitter::{Node, Parser};

use crate::parser::languages::LANGUAGES;

/// Symbol kinds
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Class,
    Interface,
    Type,
    Variable,
    Constant,
    Enum,
    Module,
    Struct,
    Trait,
    Method,
    Property,
    Unknown,
}

impl std::fmt::Display for SymbolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolKind::Function => write!(f, "function"),
            SymbolKind::Class => write!(f, "class"),
            SymbolKind::Interface => write!(f, "interface"),
            SymbolKind::Type => write!(f, "type"),
            SymbolKind::Variable => write!(f, "variable"),
            SymbolKind::Constant => write!(f, "constant"),
            SymbolKind::Enum => write!(f, "enum"),
            SymbolKind::Module => write!(f, "module"),
            SymbolKind::Struct => write!(f, "struct"),
            SymbolKind::Trait => write!(f, "trait"),
            SymbolKind::Method => write!(f, "method"),
            SymbolKind::Property => write!(f, "property"),
            SymbolKind::Unknown => write!(f, "unknown"),
        }
    }
}

/// Extracted symbol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub byte_start: Option<usize>,
    pub byte_end: Option<usize>,
    pub scope: Option<String>,
}

/// Symbol extractor using tree-sitter node traversal
pub struct SymbolExtractor;

impl Default for SymbolExtractor {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract symbols from source code
    pub fn extract(&self, source: &str, language: &str) -> Result<Vec<Symbol>> {
        let mut parser = Parser::new();
        self.extract_with_parser(source, language, &mut parser)
    }

    /// Extract symbols using a caller-provided parser instance.
    pub fn extract_with_parser(
        &self,
        source: &str,
        language: &str,
        parser: &mut Parser,
    ) -> Result<Vec<Symbol>> {
        let lang = LANGUAGES
            .get(language)
            .ok_or_else(|| anyhow::anyhow!("Unsupported language: {}", language))?;

        parser.set_language(lang)?;

        let tree = parser
            .parse(source, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse source"))?;

        let source_bytes = source.as_bytes();
        let mut symbols = Vec::new();

        self.traverse_node(tree.root_node(), source_bytes, language, &mut symbols);

        if matches!(language, "c" | "cpp") {
            let mut seen = HashSet::new();
            for symbol in &symbols {
                seen.insert(symbol_dedupe_key(symbol));
            }
            for extra in self.extract_c_like_type_declarations(source) {
                if seen.insert(symbol_dedupe_key(&extra)) {
                    symbols.push(extra);
                }
            }
        }

        dedupe_symbols_in_place(&mut symbols);

        Ok(symbols)
    }

    /// Extract symbols while reusing parser instances per language.
    pub fn extract_with_cache(
        &self,
        source: &str,
        language: &str,
        cache: &mut HashMap<String, Parser>,
    ) -> Result<Vec<Symbol>> {
        use std::collections::hash_map::Entry;

        let parser = match cache.entry(language.to_string()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Parser::new()),
        };
        self.extract_with_parser(source, language, parser)
    }

    /// Traverse the AST and extract symbols
    fn traverse_node(&self, node: Node, source: &[u8], lang: &str, symbols: &mut Vec<Symbol>) {
        // Extract symbol based on node type and language
        if let Some(symbol) = self.extract_symbol_from_node(node, source, lang) {
            symbols.push(symbol);
        }

        // Recursively traverse children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.traverse_node(child, source, lang, symbols);
        }
    }

    /// Extract a symbol from a node if it represents a definition
    fn extract_symbol_from_node(&self, node: Node, source: &[u8], lang: &str) -> Option<Symbol> {
        let kind = node.kind();

        if matches!(lang, "c" | "cpp")
            && kind == "function_declarator"
            && is_inside_c_like_function_definition(node)
        {
            return None;
        }

        // Match patterns based on language
        let (symbol_kind, name_field) = match lang {
            "typescript" | "javascript" => self.match_typescript_node(kind),
            "python" => self.match_python_node(kind),
            "rust" => self.match_rust_node(kind),
            "go" => self.match_go_node(kind),
            "c" => self.match_c_node(kind),
            "cpp" => self.match_cpp_node(kind),
            "java" => self.match_java_node(kind),
            "ruby" => self.match_ruby_node(kind),
            _ => return None,
        }?;

        let raw_name = self.extract_name_text(node, source, lang, kind, name_field)?;
        let mut effective_kind = symbol_kind;
        let mut effective_raw_name = raw_name;

        if matches!(lang, "c" | "cpp") {
            if matches!(effective_kind, SymbolKind::Function | SymbolKind::Method) {
                if let Some((decl_kind, decl_name)) =
                    parse_c_like_type_name_with_kind(&effective_raw_name)
                {
                    effective_kind = decl_kind;
                    effective_raw_name = decl_name;
                }
            }

            if matches!(
                effective_kind,
                SymbolKind::Class | SymbolKind::Struct | SymbolKind::Enum
            ) && (looks_like_cpp_macro(effective_raw_name.trim())
                || is_cpp_decl_keyword(effective_raw_name.trim()))
            {
                if let Some(parsed_name) = extract_c_like_type_name(node, source, kind) {
                    effective_raw_name = parsed_name;
                }
            }
        }

        let name = self.normalize_symbol_name(effective_raw_name, lang, &effective_kind);
        if name.is_empty() {
            return None;
        }

        Some(Symbol {
            name,
            kind: effective_kind,
            line: node.start_position().row + 1,
            column: node.start_position().column + 1,
            end_line: node.end_position().row + 1,
            byte_start: Some(node.start_byte()),
            byte_end: Some(node.end_byte()),
            scope: None,
        })
    }

    /// Match TypeScript/JavaScript AST nodes
    fn match_typescript_node(&self, kind: &str) -> Option<(SymbolKind, &'static str)> {
        match kind {
            "function_declaration" => Some((SymbolKind::Function, "name")),
            "class_declaration" => Some((SymbolKind::Class, "name")),
            "interface_declaration" => Some((SymbolKind::Interface, "name")),
            "type_alias_declaration" => Some((SymbolKind::Type, "name")),
            "enum_declaration" => Some((SymbolKind::Enum, "name")),
            "method_definition" => Some((SymbolKind::Method, "name")),
            "variable_declarator" => Some((SymbolKind::Variable, "name")),
            _ => None,
        }
    }

    fn extract_name_text(
        &self,
        node: Node,
        source: &[u8],
        lang: &str,
        node_kind: &str,
        name_field: &str,
    ) -> Option<String> {
        if let Some(name_node) = node.child_by_field_name(name_field) {
            if let Ok(text) = name_node.utf8_text(source) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        self.fallback_name_text(node, source, lang, node_kind)
    }

    fn fallback_name_text(
        &self,
        node: Node,
        source: &[u8],
        lang: &str,
        node_kind: &str,
    ) -> Option<String> {
        if !matches!(lang, "c" | "cpp") {
            return None;
        }
        match node_kind {
            // C++ type declarations with visibility/attribute macros may omit the
            // `name` field in some tree-sitter forms.
            "class_specifier" | "struct_specifier" | "enum_specifier" => {
                extract_c_like_type_name(node, source, node_kind).or_else(|| {
                    self.find_first_named_descendant(
                        node,
                        source,
                        &["type_identifier", "identifier", "qualified_identifier"],
                        6,
                    )
                })
            }
            _ => None,
        }
    }

    fn find_first_named_descendant(
        &self,
        node: Node,
        source: &[u8],
        target_kinds: &[&str],
        max_depth: usize,
    ) -> Option<String> {
        if max_depth == 0 {
            return None;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if target_kinds.iter().any(|kind| *kind == child.kind()) {
                if let Ok(text) = child.utf8_text(source) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
            if let Some(found) =
                self.find_first_named_descendant(child, source, target_kinds, max_depth - 1)
            {
                return Some(found);
            }
        }
        None
    }

    fn normalize_symbol_name(&self, raw: String, lang: &str, kind: &SymbolKind) -> String {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        if matches!(lang, "c" | "cpp") && matches!(kind, SymbolKind::Function | SymbolKind::Method)
        {
            return canonicalize_c_like_function_name(trimmed);
        }
        trimmed.to_string()
    }

    fn extract_c_like_type_declarations(&self, source: &str) -> Vec<Symbol> {
        let mut symbols = Vec::new();
        let lines: Vec<&str> = source.lines().collect();
        let candidates = [
            ("class", SymbolKind::Class),
            ("struct", SymbolKind::Struct),
            ("enum", SymbolKind::Enum),
        ];

        for (idx, raw_line) in lines.iter().enumerate() {
            let line = strip_line_comment(raw_line).trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            for (keyword, kind) in candidates.iter() {
                if !line.contains(keyword) {
                    continue;
                }
                let Some(name) = parse_type_name_from_declaration(line, keyword) else {
                    continue;
                };
                if looks_like_cpp_macro(&name) {
                    continue;
                }
                if !is_probable_type_definition(&lines, idx, line) {
                    continue;
                }
                let column = line.find(&name).map(|col| col + 1).unwrap_or(1);
                symbols.push(Symbol {
                    name,
                    kind: kind.clone(),
                    line: idx + 1,
                    column,
                    end_line: idx + 1,
                    byte_start: None,
                    byte_end: None,
                    scope: None,
                });
                break;
            }
        }
        symbols
    }

    /// Match Python AST nodes
    fn match_python_node(&self, kind: &str) -> Option<(SymbolKind, &'static str)> {
        match kind {
            "function_definition" => Some((SymbolKind::Function, "name")),
            "class_definition" => Some((SymbolKind::Class, "name")),
            _ => None,
        }
    }

    /// Match Rust AST nodes
    fn match_rust_node(&self, kind: &str) -> Option<(SymbolKind, &'static str)> {
        match kind {
            "function_item" => Some((SymbolKind::Function, "name")),
            "struct_item" => Some((SymbolKind::Struct, "name")),
            "enum_item" => Some((SymbolKind::Enum, "name")),
            "trait_item" => Some((SymbolKind::Trait, "name")),
            "type_item" => Some((SymbolKind::Type, "name")),
            "const_item" => Some((SymbolKind::Constant, "name")),
            "static_item" => Some((SymbolKind::Variable, "name")),
            "mod_item" => Some((SymbolKind::Module, "name")),
            _ => None,
        }
    }

    /// Match Go AST nodes
    fn match_go_node(&self, kind: &str) -> Option<(SymbolKind, &'static str)> {
        match kind {
            "function_declaration" => Some((SymbolKind::Function, "name")),
            "method_declaration" => Some((SymbolKind::Method, "name")),
            "type_spec" => Some((SymbolKind::Type, "name")),
            _ => None,
        }
    }

    /// Match C AST nodes
    fn match_c_node(&self, kind: &str) -> Option<(SymbolKind, &'static str)> {
        match kind {
            "function_definition" => Some((SymbolKind::Function, "declarator")),
            "function_declarator" => Some((SymbolKind::Function, "declarator")),
            "struct_specifier" => Some((SymbolKind::Struct, "name")),
            "enum_specifier" => Some((SymbolKind::Enum, "name")),
            "type_definition" => Some((SymbolKind::Type, "declarator")),
            _ => None,
        }
    }

    /// Match C++ AST nodes
    fn match_cpp_node(&self, kind: &str) -> Option<(SymbolKind, &'static str)> {
        match kind {
            "function_definition" => Some((SymbolKind::Function, "declarator")),
            "function_declarator" => Some((SymbolKind::Function, "declarator")),
            "class_specifier" => Some((SymbolKind::Class, "name")),
            "struct_specifier" => Some((SymbolKind::Struct, "name")),
            "enum_specifier" => Some((SymbolKind::Enum, "name")),
            "namespace_definition" => Some((SymbolKind::Module, "name")),
            "type_definition" => Some((SymbolKind::Type, "declarator")),
            _ => None,
        }
    }

    /// Match Java AST nodes
    fn match_java_node(&self, kind: &str) -> Option<(SymbolKind, &'static str)> {
        match kind {
            "method_declaration" => Some((SymbolKind::Method, "name")),
            "class_declaration" => Some((SymbolKind::Class, "name")),
            "interface_declaration" => Some((SymbolKind::Interface, "name")),
            "enum_declaration" => Some((SymbolKind::Enum, "name")),
            "constructor_declaration" => Some((SymbolKind::Function, "name")),
            "field_declaration" => Some((SymbolKind::Property, "declarator")),
            _ => None,
        }
    }

    /// Match Ruby AST nodes
    fn match_ruby_node(&self, kind: &str) -> Option<(SymbolKind, &'static str)> {
        match kind {
            "method" => Some((SymbolKind::Method, "name")),
            "singleton_method" => Some((SymbolKind::Method, "name")),
            "class" => Some((SymbolKind::Class, "name")),
            "module" => Some((SymbolKind::Module, "name")),
            _ => None,
        }
    }
}

fn canonicalize_c_like_function_name(raw: &str) -> String {
    let mut head = raw.trim();
    if let Some(paren_idx) = head.find('(') {
        let prefix = head[..paren_idx].trim();
        if prefix
            .chars()
            .any(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '~'))
        {
            head = prefix;
        }
    }

    let mut cleaned = head.trim();
    cleaned = cleaned.trim_start_matches(['&', '*']);
    cleaned = cleaned.trim();
    cleaned = cleaned.trim_end_matches(['&', '*']);
    cleaned = cleaned.trim();
    cleaned = cleaned.trim_matches(['(', ')']);

    let token = cleaned
        .split_whitespace()
        .last()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(cleaned);
    token.to_string()
}

fn symbol_dedupe_key(symbol: &Symbol) -> String {
    format!(
        "{}:{}:{}:{}:{}",
        symbol.kind,
        symbol.line,
        symbol.column,
        symbol.end_line,
        symbol.name.to_ascii_lowercase()
    )
}

fn dedupe_symbols_in_place(symbols: &mut Vec<Symbol>) {
    let mut seen = HashSet::new();
    symbols.retain(|symbol| seen.insert(symbol_dedupe_key(symbol)));
}

fn is_inside_c_like_function_definition(node: Node) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "function_definition" {
            return true;
        }
        current = parent.parent();
    }
    false
}

fn parse_c_like_type_name_with_kind(raw: &str) -> Option<(SymbolKind, String)> {
    let candidates = [
        ("class", SymbolKind::Class),
        ("struct", SymbolKind::Struct),
        ("enum", SymbolKind::Enum),
    ];
    for (keyword, kind) in candidates {
        if !raw.contains(keyword) {
            continue;
        }
        if let Some(name) = parse_type_name_from_declaration(raw, keyword) {
            return Some((kind, name));
        }
    }
    None
}

fn strip_line_comment(raw_line: &str) -> &str {
    raw_line.split("//").next().unwrap_or(raw_line)
}

fn is_probable_type_definition(lines: &[&str], idx: usize, line: &str) -> bool {
    if line.contains('{') {
        return true;
    }
    if line.ends_with(';') {
        return false;
    }
    for offset in 1..=3 {
        let Some(next_raw) = lines.get(idx + offset) else {
            break;
        };
        let next = strip_line_comment(next_raw).trim();
        if next.is_empty() {
            continue;
        }
        return next.starts_with('{');
    }
    false
}

fn extract_c_like_type_name(node: Node, source: &[u8], node_kind: &str) -> Option<String> {
    let keyword = match node_kind {
        "class_specifier" => "class",
        "struct_specifier" => "struct",
        "enum_specifier" => "enum",
        _ => return None,
    };
    let text = node.utf8_text(source).ok()?;
    parse_type_name_from_declaration(text, keyword)
}

fn parse_type_name_from_declaration(text: &str, keyword: &str) -> Option<String> {
    let header = text.split('{').next().unwrap_or(text);
    let after_keyword = if let Some((_, tail)) = header.split_once(keyword) {
        tail
    } else {
        return None;
    };
    for token in after_keyword
        .split(|ch: char| {
            ch.is_whitespace() || matches!(ch, ':' | ';' | ',' | '<' | '>' | '(' | ')' | '[' | ']')
        })
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        if is_cpp_decl_keyword(token) || looks_like_cpp_macro(token) {
            continue;
        }
        return Some(token.to_string());
    }
    None
}

fn is_cpp_decl_keyword(token: &str) -> bool {
    matches!(
        token,
        "class"
            | "struct"
            | "enum"
            | "union"
            | "final"
            | "public"
            | "private"
            | "protected"
            | "virtual"
            | "constexpr"
            | "typename"
            | "template"
            | "const"
            | "volatile"
            | "friend"
    )
}

fn looks_like_cpp_macro(token: &str) -> bool {
    if token.starts_with("__") {
        return true;
    }
    token
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_typescript_function() {
        let source = r#"
function greet(name: string): string {
    return `Hello, ${name}!`;
}
"#;
        let extractor = SymbolExtractor::new();
        let symbols = extractor.extract(source, "typescript").unwrap();

        assert!(!symbols.is_empty());
        let func = symbols.iter().find(|s| s.name == "greet").unwrap();
        assert_eq!(func.kind, SymbolKind::Function);
    }

    #[test]
    fn test_extract_typescript_class() {
        let source = r#"
class Person {
    constructor(public name: string) {}
    
    greet(): string {
        return `Hello, ${this.name}!`;
    }
}
"#;
        let extractor = SymbolExtractor::new();
        let symbols = extractor.extract(source, "typescript").unwrap();

        let class = symbols.iter().find(|s| s.name == "Person").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
    }

    #[test]
    fn test_extract_rust_function() {
        let source = r#"
fn main() {
    println!("Hello, world!");
}

pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
        let extractor = SymbolExtractor::new();
        let symbols = extractor.extract(source, "rust").unwrap();

        assert!(symbols
            .iter()
            .any(|s| s.name == "main" && s.kind == SymbolKind::Function));
        assert!(symbols
            .iter()
            .any(|s| s.name == "add" && s.kind == SymbolKind::Function));
    }

    #[test]
    fn test_extract_python_class() {
        let source = r#"
class Calculator:
    def add(self, a, b):
        return a + b
    
    def subtract(self, a, b):
        return a - b
"#;
        let extractor = SymbolExtractor::new();
        let symbols = extractor.extract(source, "python").unwrap();

        let class = symbols.iter().find(|s| s.name == "Calculator").unwrap();
        assert_eq!(class.kind, SymbolKind::Class);
    }

    #[test]
    fn test_symbol_kind_display() {
        assert_eq!(SymbolKind::Function.to_string(), "function");
        assert_eq!(SymbolKind::Class.to_string(), "class");
        assert_eq!(SymbolKind::Variable.to_string(), "variable");
    }

    #[test]
    fn test_extract_cpp_struct_with_macro_attribute() {
        let source = r#"
struct TORCH_API TensorIterator final : public TensorIteratorBase {
    TensorIterator();
};
"#;
        let extractor = SymbolExtractor::new();
        let symbols = extractor.extract(source, "cpp").unwrap();
        assert!(symbols
            .iter()
            .any(|s| s.name == "TensorIterator" && s.kind == SymbolKind::Struct));
    }

    #[test]
    fn test_cpp_function_symbol_name_is_canonicalized() {
        let source = r#"
TensorIteratorConfig& TensorIteratorConfig::add_owned_output(const TensorBase& output) {
    return *this;
}
"#;
        let extractor = SymbolExtractor::new();
        let symbols = extractor.extract(source, "cpp").unwrap();
        let function_symbols: Vec<&Symbol> = symbols
            .iter()
            .filter(|symbol| symbol.kind == SymbolKind::Function)
            .collect();
        assert!(function_symbols
            .iter()
            .any(|symbol| symbol.name == "TensorIteratorConfig::add_owned_output"));
        assert!(function_symbols
            .iter()
            .all(|symbol| !symbol.name.contains('(')));
        assert!(function_symbols
            .iter()
            .all(|symbol| !symbol.name.starts_with('&')));
        let canonical_name = "TensorIteratorConfig::add_owned_output";
        assert_eq!(
            function_symbols
                .iter()
                .filter(|symbol| symbol.name == canonical_name)
                .count(),
            1,
            "canonicalized C++ function symbol should not be duplicated"
        );
    }

    #[test]
    fn test_unsupported_language() {
        let extractor = SymbolExtractor::new();
        let result = extractor.extract("code", "unknown_lang");
        assert!(result.is_err());
    }
}
