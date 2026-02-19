// SPDX-License-Identifier: MIT OR Apache-2.0

//! File scanner using the ignore crate (same as ripgrep)

use anyhow::Result;
use ignore::WalkBuilder;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

const INDEXABLE_EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "cpp", "cc", "h", "hpp", "cs", "rb",
    "php", "swift", "kt", "kts", "scala", "lua", "md", "txt", "json", "yaml", "toml",
];

/// Scanned file with content
#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub path: PathBuf,
    pub content: String,
    pub language: Option<String>,
}

/// File scanner that respects ignore files and custom excludes
pub struct FileScanner {
    root: PathBuf,
    exclude_patterns: Vec<String>,
    include_paths: Vec<String>,
    respect_git_ignore: bool,
    recursive: bool,
}

impl FileScanner {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            exclude_patterns: Vec::new(),
            include_paths: Vec::new(),
            respect_git_ignore: true,
            recursive: true,
        }
    }

    /// Create scanner with exclude patterns
    pub fn with_excludes(root: impl AsRef<Path>, excludes: Vec<String>) -> Self {
        let mut scanner = Self::new(root);
        scanner.exclude_patterns = excludes;
        scanner
    }

    /// Enable or disable respect for ignore files (.ignore/.gitignore)
    pub fn with_gitignore(mut self, enabled: bool) -> Self {
        self.respect_git_ignore = enabled;
        self
    }

    /// Explicit paths to include even when ignore files would normally skip them.
    pub fn with_includes(mut self, includes: Vec<String>) -> Self {
        self.include_paths = includes;
        self
    }

    /// Enable or disable recursive traversal
    pub fn with_recursive(mut self, enabled: bool) -> Self {
        self.recursive = enabled;
        self
    }

    fn make_builder(&self) -> WalkBuilder {
        let mut builder = WalkBuilder::new(&self.root);
        builder.hidden(false);
        if !self.recursive {
            builder.max_depth(Some(1));
        }

        if self.respect_git_ignore {
            builder
                .ignore(true)
                .git_ignore(true)
                .git_exclude(true)
                .git_global(true);
        } else {
            builder
                .ignore(false)
                .git_ignore(false)
                .git_exclude(false)
                .git_global(false);
        }

        builder
    }

    fn is_reserved_dir_name(name: &str) -> bool {
        matches!(name, ".cgrep" | ".git" | ".hg" | ".svn")
    }

    fn path_matches_excludes(path: &Path, exclude_patterns: &[String]) -> bool {
        if exclude_patterns.is_empty() {
            return false;
        }
        let path_str = path.to_string_lossy();
        exclude_patterns
            .iter()
            .any(|pattern| !pattern.is_empty() && path_str.contains(pattern.as_str()))
    }

    fn matches_excludes(&self, path: &Path) -> bool {
        Self::path_matches_excludes(path, &self.exclude_patterns)
    }

    fn collect_explicit_include_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for raw in &self.include_paths {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }

            let include_path = if Path::new(trimmed).is_absolute() {
                PathBuf::from(trimmed)
            } else {
                self.root.join(trimmed)
            };

            if !include_path.exists() {
                continue;
            }

            if include_path.is_file() {
                if self.matches_excludes(&include_path) {
                    continue;
                }
                if let Some(ext) = include_path.extension().and_then(|e| e.to_str()) {
                    if is_indexable_extension(ext) {
                        files.push(include_path);
                    }
                }
                continue;
            }

            let mut builder = WalkBuilder::new(&include_path);
            builder
                .hidden(false)
                .ignore(false)
                .git_ignore(false)
                .git_exclude(false)
                .git_global(false);
            let walker = builder
                .filter_entry(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .map(|name| !Self::is_reserved_dir_name(name))
                        .unwrap_or(true)
                })
                .build();

            for entry in walker {
                let Ok(entry) = entry else {
                    continue;
                };
                let path = entry.path();
                if !path.is_file() || self.matches_excludes(path) {
                    continue;
                }
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if is_indexable_extension(ext) {
                        files.push(path.to_path_buf());
                    }
                }
            }
        }
        files
    }

    fn dedupe_paths(files: Vec<PathBuf>) -> Vec<PathBuf> {
        let mut seen = HashSet::new();
        files
            .into_iter()
            .filter(|path| seen.insert(path.to_string_lossy().to_string()))
            .collect()
    }

    /// Scan all files in the directory
    pub fn scan(&self) -> Result<Vec<ScannedFile>> {
        let (tx, rx) = mpsc::channel();

        let walker = self
            .make_builder()
            .filter_entry(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|name| !Self::is_reserved_dir_name(name))
                    .unwrap_or(true)
            })
            .build_parallel();

        let exclude_patterns = self.exclude_patterns.clone();
        walker.run(|| {
            let tx = tx.clone();
            let exclude_patterns = exclude_patterns.clone();

            Box::new(move |entry| {
                if let Ok(entry) = entry {
                    let path = entry.path();

                    if Self::path_matches_excludes(path, &exclude_patterns) {
                        return ignore::WalkState::Continue;
                    }

                    if path.is_file() {
                        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                            if is_indexable_extension(ext) {
                                if let Ok(content) = std::fs::read_to_string(path) {
                                    let language = detect_language(ext);
                                    let _ = tx.send(ScannedFile {
                                        path: path.to_path_buf(),
                                        content,
                                        language,
                                    });
                                }
                            }
                        }
                    }
                }
                ignore::WalkState::Continue
            })
        });

        drop(tx);
        let mut files: Vec<ScannedFile> = rx.into_iter().collect();
        let explicit_files = self.collect_explicit_include_files();
        if !explicit_files.is_empty() {
            for path in explicit_files {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let language = detect_language(ext);
                        files.push(ScannedFile {
                            path,
                            content,
                            language,
                        });
                    }
                }
            }
        }

        let mut seen = HashSet::new();
        files.retain(|file| seen.insert(file.path.to_string_lossy().to_string()));
        Ok(files)
    }

    /// Get list of file paths only (faster)
    pub fn list_files(&self) -> Result<Vec<PathBuf>> {
        let (tx, rx) = mpsc::channel();

        let walker = self
            .make_builder()
            .filter_entry(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .map(|name| !Self::is_reserved_dir_name(name))
                    .unwrap_or(true)
            })
            .build_parallel();

        let exclude_patterns = self.exclude_patterns.clone();
        walker.run(|| {
            let tx = tx.clone();
            let exclude_patterns = exclude_patterns.clone();

            Box::new(move |entry| {
                if let Ok(entry) = entry {
                    let path = entry.path();

                    if Self::path_matches_excludes(path, &exclude_patterns) {
                        return ignore::WalkState::Continue;
                    }

                    if path.is_file() {
                        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                            if is_indexable_extension(ext) {
                                let _ = tx.send(path.to_path_buf());
                            }
                        }
                    }
                }
                ignore::WalkState::Continue
            })
        });

        drop(tx);
        let mut files: Vec<PathBuf> = rx.into_iter().collect();
        files.extend(self.collect_explicit_include_files());
        Ok(Self::dedupe_paths(files))
    }
}

/// True when a file extension is included in indexing/scanning.
pub fn is_indexable_extension(ext: &str) -> bool {
    let lower = ext.to_ascii_lowercase();
    INDEXABLE_EXTENSIONS.contains(&lower.as_str())
}

/// Detect language from file extension
pub fn detect_language(ext: &str) -> Option<String> {
    match ext.to_lowercase().as_str() {
        "rs" => Some("rust".into()),
        "ts" | "tsx" => Some("typescript".into()),
        "js" | "jsx" => Some("javascript".into()),
        "py" => Some("python".into()),
        "go" => Some("go".into()),
        "java" => Some("java".into()),
        "c" | "h" => Some("c".into()),
        "cpp" | "cc" | "hpp" => Some("cpp".into()),
        "cs" => Some("csharp".into()),
        "rb" => Some("ruby".into()),
        "php" => Some("php".into()),
        "swift" => Some("swift".into()),
        "kt" | "kts" => Some("kotlin".into()),
        "scala" => Some("scala".into()),
        "lua" => Some("lua".into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_language, is_indexable_extension};

    #[test]
    fn detectable_code_extensions_are_indexable() {
        for ext in [
            "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "c", "h", "cpp", "cc", "hpp", "cs",
            "rb", "php", "swift", "kt", "kts", "scala", "lua",
        ] {
            assert!(is_indexable_extension(ext), "{ext} should be indexable");
        }
    }

    #[test]
    fn extension_aliases_map_to_expected_languages() {
        assert_eq!(detect_language("cc").as_deref(), Some("cpp"));
        assert_eq!(detect_language("kts").as_deref(), Some("kotlin"));
        assert!(is_indexable_extension("CC"));
        assert!(is_indexable_extension("KTS"));
    }
}
