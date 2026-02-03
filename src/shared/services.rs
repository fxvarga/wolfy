//! Shared Services Container
//!
//! Holds references to domain services that can be injected into components.

use crate::domain::services::FuzzyMatcher;

/// Container for shared domain services
///
/// This provides a lightweight way to inject services into components
/// without the full CompositionRoot overhead.
#[derive(Clone, Debug)]
pub struct Services {
    /// Fuzzy string matcher for search
    pub fuzzy_matcher: FuzzyMatcher,
}

impl Services {
    /// Create a new services container with default configuration
    pub fn new() -> Self {
        Self {
            fuzzy_matcher: FuzzyMatcher::new(),
        }
    }
}

impl Default for Services {
    fn default() -> Self {
        Self::new()
    }
}
