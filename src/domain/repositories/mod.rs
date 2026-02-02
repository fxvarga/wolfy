//! Domain Repository Interfaces
//!
//! These are trait definitions only - no implementations.
//! Implementations belong in the adapters/gateways layer.

pub mod app_repository;
pub mod history_repository;
pub mod icon_repository;
pub mod theme_repository;

pub use app_repository::AppRepository;
pub use history_repository::HistoryRepository;
pub use icon_repository::IconRepository;
pub use theme_repository::ThemeRepository;
