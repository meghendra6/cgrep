// SPDX-License-Identifier: MIT OR Apache-2.0

//! AST-based usage extraction helpers for callers/references.

use std::collections::HashMap;

use tree_sitter::{Node, Parser, Tree};

use crate::parser::languages::LANGUAGES;

#[derive(Debug, Clone, Copy)]
pub struct UsageMatch {
    pub line: usize,
    pub column: usize,
}

pub struct AstUsageExtractor {
    parser_cache: HashMap<String, Parser>,
}

impl AstUsageExtractor {
    pub fn new() -> Self {
        Self {
            parser_cache: HashMap::new(),
        }
    }

    pub fn references(
        &mut self,
        source: &str,
        language: &str,
        symbol: &str,
        max_results: usize,
    ) -> Option<Vec<UsageMatch>> {
        let tree = self.parse(source, language)?;
        let mut matches: Vec<UsageMatch> = Vec::new();
        let source_bytes = source.as_bytes();
        let mut seen = std::collections::HashSet::new();

        walk_tree(tree.root_node(), &mut |node| {
            if matches.len() >= max_results || !is_identifier_like(node.kind()) {
                return;
            }
            let Some(name) = identifier_name(node, source_bytes) else {
                return;
            };
            if name != symbol {
                return;
            }

            let line = node.start_position().row + 1;
            let column = node.start_position().column + 1;
            if seen.insert((line, column)) {
                matches.push(UsageMatch { line, column });
            }
        });

        Some(matches)
    }

    pub fn callers(
        &mut self,
        source: &str,
        language: &str,
        function: &str,
        max_results: usize,
    ) -> Option<Vec<UsageMatch>> {
        let tree = self.parse(source, language)?;
        let mut matches: Vec<UsageMatch> = Vec::new();
        let source_bytes = source.as_bytes();
        let mut seen_lines = std::collections::HashSet::new();

        walk_tree(tree.root_node(), &mut |node| {
            if matches.len() >= max_results || !is_call_like(node.kind()) {
                return;
            }
            let Some(callee) = call_name(node, source_bytes) else {
                return;
            };
            if callee != function {
                return;
            }

            let line = node.start_position().row + 1;
            if seen_lines.insert(line) {
                matches.push(UsageMatch {
                    line,
                    column: node.start_position().column + 1,
                });
            }
        });

        Some(matches)
    }

    fn parse(&mut self, source: &str, language: &str) -> Option<Tree> {
        let lang = LANGUAGES.get(language)?;
        use std::collections::hash_map::Entry;

        let parser = match self.parser_cache.entry(language.to_string()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(Parser::new()),
        };
        parser.set_language(lang).ok()?;
        parser.parse(source, None)
    }
}

fn walk_tree<F>(root: Node<'_>, visitor: &mut F)
where
    F: FnMut(Node<'_>),
{
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        visitor(node);
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
}

fn is_identifier_like(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "property_identifier"
            | "field_identifier"
            | "type_identifier"
            | "shorthand_property_identifier"
            | "shorthand_property_identifier_pattern"
            | "name"
    )
}

fn is_call_like(kind: &str) -> bool {
    matches!(
        kind,
        "call_expression"
            | "method_invocation"
            | "function_call_expression"
            | "function_call"
            | "call"
    )
}

fn call_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    for field in ["function", "callee", "name", "method"] {
        if let Some(child) = node.child_by_field_name(field) {
            if let Some(name) = deepest_identifier_name(child, source) {
                return Some(name);
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "arguments" | "argument_list") {
            break;
        }
        if let Some(name) = deepest_identifier_name(child, source) {
            return Some(name);
        }
    }

    None
}

fn deepest_identifier_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    if is_identifier_like(node.kind()) {
        return identifier_name(node, source);
    }

    let mut cursor = node.walk();
    let mut children: Vec<Node<'_>> = node.children(&mut cursor).collect();
    children.reverse();
    for child in children {
        if let Some(name) = deepest_identifier_name(child, source) {
            return Some(name);
        }
    }
    None
}

fn identifier_name(node: Node<'_>, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).ok()?.trim();
    if text.is_empty() {
        return None;
    }
    let text = text.trim_end_matches('?');
    let text = text
        .rsplit("::")
        .next()
        .unwrap_or(text)
        .rsplit("->")
        .next()
        .unwrap_or(text)
        .rsplit('.')
        .next()
        .unwrap_or(text)
        .trim();
    if text.is_empty() {
        return None;
    }
    Some(text.to_string())
}
