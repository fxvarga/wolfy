//! Wolfy - Application Launcher with Clean Architecture
//!
//! This crate is organized following Clean Architecture principles:
//!
//! - **Domain Layer**: Pure business logic with no external dependencies
//! - **Application Layer**: Use cases and business workflows
//! - **Adapters Layer**: Interface adapters (controllers, presenters, gateways)
//! - **Infrastructure Layer**: Concrete implementations (Win32, filesystem, etc.)
//! - **UI Layer**: Widgets, layout, and rendering
//! - **Shared**: Cross-cutting concerns (config, logging)
//!
//! The main binary is in main.rs, but we expose modules as a library
//! so we can run tests without Windows dependencies.

#[macro_use]
extern crate lalrpop_util;

// Include the log module so the log! macro works
#[macro_use]
pub mod log;

// ============================================================================
// CLEAN ARCHITECTURE LAYERS
// ============================================================================

/// Domain Layer - Pure business logic with zero external dependencies
///
/// Contains:
/// - Entities: Core business objects (AppItem, Theme, Task, WindowState)
/// - Value Objects: Immutable domain values (Color, Rect, Distance, Hotkey)
/// - Repository Traits: Data access interfaces
/// - Domain Services: Stateless business logic (FuzzyMatcher, ThemeResolver)
pub mod domain;

/// Application Layer - Use cases and business workflows
///
/// Contains:
/// - Use Cases: Single-purpose business operations (LaunchApp, SearchApps)
/// - Ports: External capability interfaces (RenderPort, RuntimePort)
/// - Application Services: Workflow coordination (CommandHandler, ThemeManager)
/// - DTOs: Data transfer objects between layers
pub mod application;

/// Adapters Layer - Interface adapters
///
/// Contains:
/// - Controllers: Translate external input to use case calls
/// - Presenters: Format data for display
/// - Gateways: Repository implementations
/// - Views: UI abstraction interfaces
pub mod adapters;

/// Infrastructure Layer - Concrete implementations
///
/// Contains:
/// - Animation: Window animation implementation
/// - Filesystem: File system operations
/// - Parser: Theme file parsing (bridges to LALRPOP)
/// - CompositionRoot: Dependency injection container
/// - Win32: Windows-specific implementations (conditional)
pub mod infrastructure;

/// UI Layer - Widgets, Layout, and Rendering
///
/// Contains:
/// - Widgets: UI component traits and implementations
/// - Layout: Layout strategies (vertical, horizontal, grid)
/// - Rendering: Render scene and primitives
pub mod ui;

/// Shared Utilities - Cross-cutting concerns
///
/// Contains:
/// - Config: Application configuration
/// - Logging: Logging utilities
pub mod shared;

// ============================================================================
// LEGACY MODULES (to be migrated)
// ============================================================================

// Expose theme module for testing (legacy - will be replaced by domain/infrastructure)
pub mod theme;

// Animation system (legacy - replaced by infrastructure::animation)
pub mod animation;

// Usage history tracking (legacy - replaced by adapters::gateways::file_history_gateway)
pub mod history;

// Task runner configuration (legacy - replaced by domain::entities::task)
pub mod tasks;

// Background task runner (has Windows dependencies for process spawning)
#[cfg(windows)]
pub mod task_runner;

// PTY module for interactive terminal sessions (Windows only)
#[cfg(windows)]
pub mod pty;

// Terminal emulator state wrapper (Windows only)
#[cfg(windows)]
pub mod terminal;

// Widget base types (legacy - replaced by ui::widgets)
pub mod widget {
    pub mod base;
    pub use base::*;
}
