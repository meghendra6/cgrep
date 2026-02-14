// SPDX-License-Identifier: MIT OR Apache-2.0

//! Smart file reading with outline fallback for large files.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::OutputFormat;
use crate::indexer::scanner::detect_language;
use crate::parser::symbols::SymbolExtractor;
use cgrep::output::print_json;

const TOKEN_THRESHOLD: u64 = 1_500;
const FILE_SIZE_CAP: u64 = 500_000;
const MAX_OUTLINE_LINES: usize = 120;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum ReadMode {
    Full,
    Outline,
    Keys,
    Section,
    Generated,
    Binary,
    Empty,
    Directory,
}

impl ReadMode {
    fn as_label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Outline => "outline",
            Self::Keys => "keys",
            Self::Section => "section",
            Self::Generated => "generated",
            Self::Binary => "binary",
            Self::Empty => "empty",
            Self::Directory => "directory",
        }
    }
}

#[derive(Debug)]
struct ReadRender {
    path: String,
    mode: ReadMode,
    size_bytes: u64,
    line_count: usize,
    tokens_estimate: u64,
    content: String,
}

#[derive(Debug, Serialize)]
struct ReadPayload<'a> {
    path: &'a str,
    mode: ReadMode,
    size_bytes: u64,
    line_count: usize,
    tokens_estimate: u64,
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct ReadJson2Meta {
    schema_version: &'static str,
    command: &'static str,
}

#[derive(Debug, Serialize)]
struct ReadJson2Payload<'a> {
    meta: ReadJson2Meta,
    result: ReadPayload<'a>,
}

/// Run the read command.
pub fn run(
    path: &str,
    section: Option<&str>,
    full: bool,
    format: OutputFormat,
    compact: bool,
) -> Result<()> {
    let cwd = std::env::current_dir().context("Cannot determine current directory")?;
    let absolute = resolve_path(&cwd, path);
    if !absolute.exists() {
        bail!("Path not found: {}", absolute.display());
    }

    let rendered = if absolute.is_dir() {
        render_directory(&cwd, &absolute)?
    } else {
        render_file(&cwd, &absolute, section, full)?
    };

    match format {
        OutputFormat::Text => {
            println!(
                "# {} ({} lines, {}) [{}]",
                rendered.path,
                rendered.line_count,
                format_token_estimate(rendered.tokens_estimate),
                rendered.mode.as_label()
            );
            if !rendered.content.is_empty() {
                println!();
                println!("{}", rendered.content);
            }
        }
        OutputFormat::Json => {
            let payload = ReadPayload {
                path: &rendered.path,
                mode: rendered.mode,
                size_bytes: rendered.size_bytes,
                line_count: rendered.line_count,
                tokens_estimate: rendered.tokens_estimate,
                content: &rendered.content,
            };
            print_json(&payload, compact)?;
        }
        OutputFormat::Json2 => {
            let payload = ReadJson2Payload {
                meta: ReadJson2Meta {
                    schema_version: "1",
                    command: "read",
                },
                result: ReadPayload {
                    path: &rendered.path,
                    mode: rendered.mode,
                    size_bytes: rendered.size_bytes,
                    line_count: rendered.line_count,
                    tokens_estimate: rendered.tokens_estimate,
                    content: &rendered.content,
                },
            };
            print_json(&payload, compact)?;
        }
    }

    Ok(())
}

fn resolve_path(cwd: &Path, raw_path: &str) -> PathBuf {
    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn display_path(cwd: &Path, path: &Path) -> String {
    path.strip_prefix(cwd).unwrap_or(path).display().to_string()
}

fn render_directory(cwd: &Path, path: &Path) -> Result<ReadRender> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).with_context(|| format!("Cannot read {}", path.display()))? {
        let entry = entry?;
        let mut name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            name.push('/');
        }
        entries.push(name);
    }
    entries.sort();

    let body = entries
        .iter()
        .map(|name| format!("  {name}"))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(ReadRender {
        path: display_path(cwd, path),
        mode: ReadMode::Directory,
        size_bytes: 0,
        line_count: entries.len(),
        tokens_estimate: estimate_tokens(body.len() as u64),
        content: body,
    })
}

fn render_file(cwd: &Path, path: &Path, section: Option<&str>, full: bool) -> Result<ReadRender> {
    let bytes = fs::read(path).with_context(|| format!("Cannot read {}", path.display()))?;
    let size_bytes = bytes.len() as u64;
    let display = display_path(cwd, path);

    if size_bytes == 0 {
        return Ok(ReadRender {
            path: display,
            mode: ReadMode::Empty,
            size_bytes,
            line_count: 0,
            tokens_estimate: 0,
            content: String::new(),
        });
    }

    if is_binary(&bytes) {
        return Ok(ReadRender {
            path: display,
            mode: ReadMode::Binary,
            size_bytes,
            line_count: 0,
            tokens_estimate: estimate_tokens(size_bytes),
            content: format!("Binary file skipped ({})", mime_from_ext(path)),
        });
    }

    let content = String::from_utf8(bytes).with_context(|| {
        format!(
            "File is not valid UTF-8 text and cannot be rendered: {}",
            path.display()
        )
    })?;
    let total_lines = line_count(&content);

    if let Some(raw_section) = section {
        let selected = select_section(path, &content, raw_section)?;
        return Ok(ReadRender {
            path: display,
            mode: ReadMode::Section,
            size_bytes,
            line_count: line_count(&selected),
            tokens_estimate: estimate_tokens(selected.len() as u64),
            content: selected,
        });
    }

    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if is_generated_file(filename, &content) {
        return Ok(ReadRender {
            path: display,
            mode: ReadMode::Generated,
            size_bytes,
            line_count: total_lines,
            tokens_estimate: estimate_tokens(size_bytes),
            content: "Generated file skipped".to_string(),
        });
    }

    let tokens = estimate_tokens(size_bytes);
    if full || tokens <= TOKEN_THRESHOLD {
        return Ok(ReadRender {
            path: display,
            mode: ReadMode::Full,
            size_bytes,
            line_count: total_lines,
            tokens_estimate: tokens,
            content,
        });
    }

    let file_type = detect_file_type(path);
    let outline = if size_bytes > FILE_SIZE_CAP {
        fallback_head_tail(&content)
    } else {
        match &file_type {
            FileType::Code(language) => code_outline(&content, language),
            FileType::Markdown => markdown_outline(&content),
            FileType::Structured => structured_outline(path, &content),
            FileType::Tabular => tabular_outline(&content),
            FileType::Log => log_outline(&content),
            FileType::Other => fallback_head_tail(&content),
        }
    };

    Ok(ReadRender {
        path: display,
        mode: if matches!(file_type, FileType::Structured) {
            ReadMode::Keys
        } else {
            ReadMode::Outline
        },
        size_bytes,
        line_count: total_lines,
        tokens_estimate: tokens,
        content: outline,
    })
}

#[derive(Debug, Clone)]
enum FileType {
    Code(String),
    Markdown,
    Structured,
    Tabular,
    Log,
    Other,
}

fn detect_file_type(path: &Path) -> FileType {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("md" | "mdx" | "rst") => FileType::Markdown,
        Some("json" | "yaml" | "yml" | "toml" | "xml" | "ini") => FileType::Structured,
        Some("csv" | "tsv") => FileType::Tabular,
        Some("log") => FileType::Log,
        Some(ext) => detect_language(ext).map_or(FileType::Other, FileType::Code),
        None => FileType::Other,
    }
}

fn code_outline(content: &str, language: &str) -> String {
    let extractor = SymbolExtractor::new();
    let symbols = match extractor.extract(content, language) {
        Ok(mut symbols) => {
            symbols.sort_by(|a, b| {
                a.line
                    .cmp(&b.line)
                    .then(a.end_line.cmp(&b.end_line))
                    .then(a.name.cmp(&b.name))
            });
            symbols
        }
        Err(_) => Vec::new(),
    };

    if symbols.is_empty() {
        return fallback_head_tail(content);
    }

    let mut out = Vec::new();
    for symbol in symbols.iter().take(MAX_OUTLINE_LINES) {
        let end_line = symbol.end_line.max(symbol.line);
        out.push(format!(
            "[{}-{}] {} {}",
            symbol.line, end_line, symbol.kind, symbol.name
        ));
    }

    if symbols.len() > out.len() {
        out.push(format!(
            "... {} more symbols omitted",
            symbols.len().saturating_sub(out.len())
        ));
    }

    out.join("\n")
}

fn markdown_outline(content: &str) -> String {
    let mut headings: Vec<(usize, usize, String)> = Vec::new();
    let mut active_fence = None;
    let lines: Vec<&str> = content.lines().collect();

    for (idx, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim_end();
        if update_code_fence_state(trimmed, &mut active_fence) {
            continue;
        }
        if active_fence.is_some() {
            continue;
        }
        let Some((level, title)) = parse_markdown_heading(trimmed) else {
            continue;
        };
        headings.push((idx + 1, level, title));
    }

    if headings.is_empty() {
        return fallback_head_tail(content);
    }

    let total_lines = lines.len();
    let mut out = Vec::new();
    for (i, (start, level, title)) in headings.iter().enumerate() {
        let mut end = total_lines;
        for (next_start, next_level, _) in headings.iter().skip(i + 1) {
            if next_level <= level {
                end = next_start.saturating_sub(1);
                break;
            }
        }
        let indent = "  ".repeat(level.saturating_sub(1));
        out.push(format!(
            "[{}-{}] {}{} {}",
            start,
            end,
            indent,
            "#".repeat(*level),
            truncate_text(title, 100)
        ));
        if out.len() >= MAX_OUTLINE_LINES {
            break;
        }
    }
    out.join("\n")
}

fn structured_outline(path: &Path, content: &str) -> String {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") => json_outline(content),
        Some("toml") => toml_outline(content),
        Some("yaml" | "yml") => yaml_outline(content),
        _ => yaml_outline(content),
    }
}

fn json_outline(content: &str) -> String {
    let parsed = match serde_json::from_str::<serde_json::Value>(content) {
        Ok(value) => value,
        Err(err) => return format!("[parse error: {err}]"),
    };

    match parsed {
        serde_json::Value::Object(map) => {
            let mut lines = Vec::new();
            for (key, value) in map.iter().take(MAX_OUTLINE_LINES) {
                lines.push(format!("{key}: {}", json_value_preview(value)));
            }
            if map.len() > lines.len() {
                lines.push(format!(
                    "... {} more keys omitted",
                    map.len().saturating_sub(lines.len())
                ));
            }
            lines.join("\n")
        }
        serde_json::Value::Array(items) => {
            format!("[array] {} items", items.len())
        }
        _ => parsed.to_string(),
    }
}

fn json_value_preview(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => format!("{{{} keys}}", map.len()),
        serde_json::Value::Array(items) => format!("[{} items]", items.len()),
        serde_json::Value::String(text) => truncate_text(text, 60),
        _ => truncate_text(&value.to_string(), 60),
    }
}

fn toml_outline(content: &str) -> String {
    let parsed = match content.parse::<toml::Value>() {
        Ok(value) => value,
        Err(err) => return format!("[parse error: {err}]"),
    };

    let Some(table) = parsed.as_table() else {
        return truncate_text(&parsed.to_string(), 200).to_string();
    };

    let mut lines = Vec::new();
    for (key, value) in table.iter().take(MAX_OUTLINE_LINES) {
        let preview = match value {
            toml::Value::Table(inner) => format!("{{{} keys}}", inner.len()),
            toml::Value::Array(items) => format!("[{} items]", items.len()),
            _ => truncate_text(&value.to_string(), 60).to_string(),
        };
        lines.push(format!("{key}: {preview}"));
    }
    if table.len() > lines.len() {
        lines.push(format!(
            "... {} more keys omitted",
            table.len().saturating_sub(lines.len())
        ));
    }
    lines.join("\n")
}

fn yaml_outline(content: &str) -> String {
    let mut lines_out = Vec::new();
    for (idx, raw) in content.lines().enumerate() {
        if lines_out.len() >= MAX_OUTLINE_LINES {
            break;
        }
        let trimmed = raw.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        if key.contains(' ') {
            continue;
        }
        let indent = raw.len().saturating_sub(trimmed.len()) / 2;
        let suffix = if value.trim().is_empty() {
            String::new()
        } else {
            format!(": {}", truncate_text(value.trim(), 60))
        };
        lines_out.push(format!(
            "[{}] {}{}{}",
            idx + 1,
            "  ".repeat(indent.min(4)),
            key.trim(),
            suffix
        ));
    }

    if lines_out.is_empty() {
        fallback_head_tail(content)
    } else {
        lines_out.join("\n")
    }
}

fn tabular_outline(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return String::new();
    }

    let mut out = Vec::new();
    out.push(format!("columns: {}", lines[0]));
    out.push(format!("rows: {}", lines.len().saturating_sub(1)));
    out.push(String::new());

    let head_end = usize::min(lines.len(), 6);
    for line in &lines[1..head_end] {
        out.push((*line).to_string());
    }
    if lines.len() > 9 {
        out.push(format!("... {} rows omitted", lines.len() - 9));
        out.push(String::new());
        for line in &lines[lines.len() - 3..] {
            out.push((*line).to_string());
        }
    } else if lines.len() > head_end {
        for line in &lines[head_end..] {
            out.push((*line).to_string());
        }
    }
    out.join("\n")
}

fn log_outline(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= 15 {
        return content.to_string();
    }

    let mut out = Vec::new();
    out.extend(lines.iter().take(10).map(|line| (*line).to_string()));
    out.push(String::new());
    out.push(format!(
        "... {} lines total, {} omitted",
        lines.len(),
        lines.len() - 15
    ));
    out.push(String::new());
    out.extend(
        lines
            .iter()
            .skip(lines.len() - 5)
            .map(|line| (*line).to_string()),
    );
    out.join("\n")
}

fn fallback_head_tail(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= 60 {
        return content.to_string();
    }

    let mut out = Vec::new();
    out.extend(lines.iter().take(50).map(|line| (*line).to_string()));
    out.push(String::new());
    out.push(format!(
        "... {} lines total, {} omitted",
        lines.len(),
        lines.len() - 60
    ));
    out.push(String::new());
    out.extend(
        lines
            .iter()
            .skip(lines.len().saturating_sub(10))
            .map(|line| (*line).to_string()),
    );
    out.join("\n")
}

fn select_section(path: &Path, content: &str, section: &str) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Ok(String::new());
    }

    let (start, end) = if section.starts_with('#') {
        resolve_heading_range(&lines, section).with_context(|| {
            format!(
                "Heading not found in {}: {}",
                path.display(),
                section.trim()
            )
        })?
    } else {
        parse_line_range(section)
            .with_context(|| format!("Invalid section format: {section} (expected start-end)"))?
    };

    let start_idx = start.saturating_sub(1);
    let end_idx = end.min(lines.len());
    if start_idx >= end_idx {
        bail!(
            "Section range is empty or out of bounds: {} (file has {} lines)",
            section,
            lines.len()
        );
    }

    Ok(lines[start_idx..end_idx].join("\n"))
}

fn parse_line_range(input: &str) -> Option<(usize, usize)> {
    let (a, b) = input.split_once('-')?;
    let start: usize = a.trim().parse().ok()?;
    let end: usize = b.trim().parse().ok()?;
    if start == 0 || end == 0 || end < start {
        return None;
    }
    Some((start, end))
}

fn update_code_fence_state(line: &str, active_fence: &mut Option<char>) -> bool {
    let trimmed = line.trim_start();
    let marker = if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    };

    if let Some(ch) = marker {
        if active_fence.is_none() {
            *active_fence = Some(ch);
            return true;
        }
        if *active_fence == Some(ch) {
            *active_fence = None;
            return true;
        }
    }

    false
}

fn parse_markdown_heading(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }

    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }

    let body = trimmed[level..].trim();
    if body.is_empty() {
        return None;
    }

    let title = strip_optional_closing_hashes(body).trim();
    if title.is_empty() {
        return None;
    }

    Some((level, title.to_string()))
}

fn strip_optional_closing_hashes(input: &str) -> &str {
    let trimmed = input.trim_end();
    let bytes = trimmed.as_bytes();

    let mut hash_start = bytes.len();
    while hash_start > 0 && bytes[hash_start - 1] == b'#' {
        hash_start -= 1;
    }

    if hash_start == bytes.len() {
        return trimmed;
    }
    if hash_start == 0 {
        return "";
    }
    if bytes[hash_start - 1].is_ascii_whitespace() {
        return trimmed[..hash_start - 1].trim_end();
    }

    trimmed
}

fn resolve_heading_range(lines: &[&str], heading: &str) -> Option<(usize, usize)> {
    let (target_level, target_title) = parse_markdown_heading(heading)?;
    let mut active_fence = None;
    let mut start_idx = None;

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim_end();
        if update_code_fence_state(trimmed, &mut active_fence) {
            continue;
        }
        if active_fence.is_some() {
            continue;
        }
        let Some((level, title)) = parse_markdown_heading(trimmed) else {
            continue;
        };
        if level == target_level && title == target_title {
            start_idx = Some(idx);
            break;
        }
    }

    let start_idx = start_idx?;
    active_fence = None;

    for (idx, line) in lines.iter().enumerate().skip(start_idx + 1) {
        let trimmed = line.trim_end();
        if update_code_fence_state(trimmed, &mut active_fence) {
            continue;
        }
        if active_fence.is_some() {
            continue;
        }
        let Some((next_level, _)) = parse_markdown_heading(trimmed) else {
            continue;
        };
        if next_level <= target_level {
            return Some((start_idx + 1, idx));
        }
    }

    Some((start_idx + 1, lines.len()))
}

fn line_count(content: &str) -> usize {
    if content.is_empty() {
        0
    } else {
        content.lines().count()
    }
}

fn estimate_tokens(bytes: u64) -> u64 {
    bytes.div_ceil(4)
}

fn format_token_estimate(tokens: u64) -> String {
    if tokens >= 1_000 {
        format!("~{}.{}k tokens", tokens / 1_000, (tokens % 1_000) / 100)
    } else {
        format!("~{tokens} tokens")
    }
}

fn truncate_text(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out = String::new();
    for ch in text.chars().take(max.saturating_sub(3)) {
        out.push(ch);
    }
    out.push_str("...");
    out
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(512).any(|b| *b == 0)
}

fn is_generated_file(name: &str, content: &str) -> bool {
    if matches!(
        name,
        "package-lock.json"
            | "yarn.lock"
            | "pnpm-lock.yaml"
            | "Cargo.lock"
            | "composer.lock"
            | "Gemfile.lock"
            | "poetry.lock"
            | "go.sum"
            | "bun.lockb"
    ) {
        return true;
    }

    let preview = if content.len() > 512 {
        &content[..512]
    } else {
        content
    };
    let preview_lower = preview.to_lowercase();
    [
        "@generated",
        "do not edit",
        "auto-generated",
        "this file is generated",
        "automatically generated",
    ]
    .iter()
    .any(|needle| preview_lower.contains(needle))
}

fn mime_from_ext(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        Some("gz" | "tgz") => "application/gzip",
        Some("tar") => "application/x-tar",
        Some("mp3") => "audio/mpeg",
        Some("mp4") => "video/mp4",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_line_range_valid() {
        assert_eq!(parse_line_range("1-3"), Some((1, 3)));
        assert_eq!(parse_line_range("10 - 42"), Some((10, 42)));
    }

    #[test]
    fn parse_line_range_invalid() {
        assert_eq!(parse_line_range("0-3"), None);
        assert_eq!(parse_line_range("9-2"), None);
        assert_eq!(parse_line_range("x-y"), None);
    }

    #[test]
    fn resolve_heading_ignores_code_block() {
        let lines = vec!["# A", "```", "## B", "```", "## C"];
        assert_eq!(resolve_heading_range(&lines, "## B"), None);
        assert_eq!(resolve_heading_range(&lines, "# A"), Some((1, 5)));
        assert_eq!(resolve_heading_range(&lines, "## C"), Some((5, 5)));
    }

    #[test]
    fn resolve_heading_matches_indented_and_closing_hashes() {
        let lines = vec!["   ## Config ##   ", "value: true", "## Next"];
        assert_eq!(resolve_heading_range(&lines, "## Config"), Some((1, 2)));
        assert_eq!(resolve_heading_range(&lines, "## Config ##"), Some((1, 2)));
    }

    #[test]
    fn resolve_heading_ignores_tilde_fence() {
        let lines = vec!["# A", "~~~", "## B", "~~~", "## C"];
        assert_eq!(resolve_heading_range(&lines, "## B"), None);
        assert_eq!(resolve_heading_range(&lines, "## C"), Some((5, 5)));
    }

    #[test]
    fn markdown_outline_ignores_tilde_fenced_headings() {
        let input = "# A\n~~~\n## B\n~~~\n## C\n";
        let out = markdown_outline(input);
        assert!(out.contains("# A"));
        assert!(!out.contains("## B"));
        assert!(out.contains("## C"));
    }

    #[test]
    fn yaml_outline_has_basic_keys() {
        let input = "name: cgrep\nnested:\n  key: value\n";
        let out = yaml_outline(input);
        assert!(out.contains("name"));
        assert!(out.contains("nested"));
    }

    #[test]
    fn json_outline_describes_arrays() {
        let input = r#"{"items":[1,2,3]}"#;
        let out = json_outline(input);
        assert!(out.contains("[3 items]"));
    }
}
