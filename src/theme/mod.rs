//! Theme module - parser and styling system

pub mod ast;
pub mod lexer;
pub mod tree;
pub mod types;

// The lalrpop generated parser (generated at build time)
// lalrpop generates to $OUT_DIR/theme/theme.rs from src/theme/theme.lalrpop
lalrpop_mod!(#[allow(clippy::all)] pub theme_parser, "/theme/theme.rs");

// Public API re-exports
pub use ast::{Property, Rule, Selector, Stylesheet, Value};
pub use lexer::{Lexer, LexerError, Token};
pub use tree::{ThemeError, ThemeNode, ThemeTree};
pub use types::{Border, Color, Distance, DistanceUnit, LayoutContext, Padding, Rect};
