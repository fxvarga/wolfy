//! SearchQuery value object - represents a search query
//!
//! Encapsulates search query text and options.

/// A search query with options
#[derive(Clone, Debug, PartialEq)]
pub struct SearchQuery {
    /// The raw query text
    pub text: String,
    /// Normalized/lowercase version for matching
    normalized: String,
    /// Whether to use fuzzy matching
    pub fuzzy: bool,
    /// Maximum number of results to return
    pub max_results: usize,
}

impl SearchQuery {
    /// Default maximum results
    pub const DEFAULT_MAX_RESULTS: usize = 50;

    /// Create a new search query
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let normalized = text.to_lowercase();

        Self {
            text,
            normalized,
            fuzzy: true,
            max_results: Self::DEFAULT_MAX_RESULTS,
        }
    }

    /// Create an empty query
    pub fn empty() -> Self {
        Self::new("")
    }

    /// Get the normalized (lowercase) query text
    pub fn normalized(&self) -> &str {
        &self.normalized
    }

    /// Check if the query is empty
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// Get individual words/tokens in the query
    pub fn tokens(&self) -> Vec<&str> {
        self.normalized.split_whitespace().collect()
    }

    /// Builder: set fuzzy matching
    pub fn with_fuzzy(mut self, fuzzy: bool) -> Self {
        self.fuzzy = fuzzy;
        self
    }

    /// Builder: set max results
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }

    /// Check if this query matches a target string (simple contains match)
    pub fn matches(&self, target: &str) -> bool {
        if self.is_empty() {
            return true;
        }

        let target_lower = target.to_lowercase();

        if self.fuzzy {
            // All tokens must match somewhere
            self.tokens().iter().all(|token| target_lower.contains(token))
        } else {
            // Exact substring match
            target_lower.contains(&self.normalized)
        }
    }
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self::empty()
    }
}

impl From<&str> for SearchQuery {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for SearchQuery {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_creation() {
        let q = SearchQuery::new("Visual Studio");

        assert_eq!(q.text, "Visual Studio");
        assert_eq!(q.normalized(), "visual studio");
    }

    #[test]
    fn test_query_empty() {
        let q = SearchQuery::empty();

        assert!(q.is_empty());
        assert!(q.matches("anything"));
    }

    #[test]
    fn test_query_tokens() {
        let q = SearchQuery::new("visual studio code");
        let tokens = q.tokens();

        assert_eq!(tokens, vec!["visual", "studio", "code"]);
    }

    #[test]
    fn test_query_matches_fuzzy() {
        let q = SearchQuery::new("studio code");

        assert!(q.matches("Visual Studio Code")); // contains both "studio" and "code"
        assert!(q.matches("Studio Code Editor"));
        assert!(!q.matches("Visual Basic")); // Missing "code" and "studio"
    }

    #[test]
    fn test_query_matches_exact() {
        let q = SearchQuery::new("studio").with_fuzzy(false);

        assert!(q.matches("Visual Studio"));
        assert!(!q.matches("VS Code"));
    }

    #[test]
    fn test_query_from() {
        let q: SearchQuery = "test".into();
        assert_eq!(q.text, "test");
    }
}
