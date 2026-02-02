//! UI Layer - Widgets, Layout, and Rendering
//!
//! This layer contains UI-specific code:
//! - **Widgets**: UI components (textbox, listview, container, etc.)
//! - **Layout**: Layout engine and strategies
//! - **Rendering**: Render scene and primitives
//!
//! This layer uses the adapters layer for data and the infrastructure layer
//! for actual rendering.

pub mod layout;
pub mod rendering;
pub mod widgets;

pub use layout::*;
pub use rendering::*;
pub use widgets::*;
