//! Domain Entities - Core business objects
//!
//! Entities are objects with a distinct identity that persists over time.
//! They represent the core business concepts of the application.

pub mod app_item;
pub mod task;
pub mod theme;
pub mod window_state;

pub use app_item::AppItem;
pub use task::Task;
pub use theme::{Theme, ThemeValue};
pub use window_state::{WindowState, WindowVisibility};
