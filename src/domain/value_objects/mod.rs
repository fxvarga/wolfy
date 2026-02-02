//! Domain Value Objects - Immutable values that describe characteristics
//!
//! Value objects have no identity and are compared by their values.
//! They are immutable and can be freely shared.

pub mod color;
pub mod dimensions;
pub mod hotkey;
pub mod rect;
pub mod search_query;

pub use color::Color;
pub use dimensions::{Distance, DistanceUnit, Padding, Size};
pub use hotkey::{Hotkey, KeyCode, Modifiers};
pub use rect::Rect;
pub use search_query::SearchQuery;
