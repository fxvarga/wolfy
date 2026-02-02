//! Domain Services - Complex operations that don't belong to a single entity
//!
//! Domain services contain business logic that operates on multiple entities
//! or doesn't naturally fit within a single entity.

pub mod fuzzy_matcher;
pub mod search_service;
pub mod theme_resolver;

pub use fuzzy_matcher::FuzzyMatcher;
pub use search_service::SearchService;
pub use theme_resolver::ThemeResolver;
