//! Error types with helpful suggestions
//!
//! Provides user-friendly error messages with actionable suggestions.

use std::fmt;

/// Error indicating the search index was not found
#[derive(Debug)]
pub struct IndexNotFoundError {
    pub index_path: String,
}

impl fmt::Display for IndexNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Index not found at '{}'\n\n\
             Suggestion: Run 'cgrep index' to create the search index first.\n\
             Example: cgrep index\n\
             Or with a specific path: cgrep index /path/to/project",
            self.index_path
        )
    }
}

impl std::error::Error for IndexNotFoundError {}

/// Error indicating no search results were found
#[derive(Debug)]
pub struct NoResultsError {
    pub query: String,
}

impl fmt::Display for NoResultsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "No results found for query: '{}'\n\n\
             Suggestions:\n\
             - Try a different or broader search query\n\
             - Check if the index is up to date: cgrep index --force\n\
             - Use --fuzzy for approximate matching: cgrep search --fuzzy \"{}\"",
            self.query, self.query
        )
    }
}

impl std::error::Error for NoResultsError {}

/// Error indicating an unsupported language was specified
#[derive(Debug)]
pub struct UnsupportedLanguageError {
    pub language: String,
    pub supported: Vec<String>,
}

impl fmt::Display for UnsupportedLanguageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let supported_list = self.supported.join(", ");
        write!(
            f,
            "Unsupported language: '{}'\n\n\
             Supported languages: {}\n\n\
             Example: cgrep symbols \"function_name\" --lang rust",
            self.language, supported_list
        )
    }
}

impl std::error::Error for UnsupportedLanguageError {}

/// Helper functions for creating helpful error messages
pub mod suggestions {
    /// Get a formatted list of supported languages
    pub fn supported_languages_message(languages: &[&str]) -> String {
        format!(
            "Supported languages: {}\n\n\
             Use the -t/--lang flag to specify a language.\n\
             Example: cgrep symbols \"main\" --lang rust",
            languages.join(", ")
        )
    }

    /// Get suggestion for index not found
    pub fn index_not_found_suggestion(path: &str) -> String {
        format!(
            "Index not found at '{}'\n\n\
             Run 'cgrep index' to create the search index:\n\
             $ cgrep index\n\n\
             Or specify a path:\n\
             $ cgrep index {}",
            path, path
        )
    }

    /// Get suggestion for no results
    pub fn no_results_suggestion(query: &str) -> String {
        format!(
            "No results found for '{}'\n\n\
             Try:\n\
             - A different search query\n\
             - Running 'cgrep index --force' to rebuild the index\n\
             - Using --fuzzy for approximate matching",
            query
        )
    }
}
