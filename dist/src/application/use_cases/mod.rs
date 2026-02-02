//! Application Use Cases - Single-purpose business operations
//!
//! Each use case encapsulates a single business operation.

pub mod launch_app;
pub mod run_task;
pub mod search_apps;
pub mod switch_theme;

pub use launch_app::LaunchAppUseCase;
pub use run_task::RunTaskUseCase;
pub use search_apps::SearchAppsUseCase;
pub use switch_theme::SwitchThemeUseCase;
