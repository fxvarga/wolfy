# AGENTS.md - Wolfy Development Guide

This document provides guidance for AI coding agents working on the Wolfy codebase.

## Project Overview

Wolfy is a Windows application launcher inspired by rofi, built in Rust. It uses:
- Direct2D for GPU-accelerated rendering
- LALRPOP for parsing rofi-compatible `.rasi` theme files
- Win32 API for window management, hotkeys, and system integration

## Build Commands

```bash
# Build (debug)
cargo build

# Build (release - optimized with LTO)
cargo build --release

# Run
cargo run

# Run release
cargo run --release

# Check for errors without building
cargo check

# Format code
cargo fmt

# Lint with clippy
cargo clippy
```

## Test Commands

```bash
# Run all tests
cargo test

# Run all tests with output
cargo test -- --nocapture

# Run a single test by name
cargo test test_linear_easing

# Run tests matching a pattern
cargo test test_color

# Run tests in a specific module
cargo test theme::tree::tests

# Run tests with backtrace on failure
RUST_BACKTRACE=1 cargo test
```

## Project Structure

```
src/
  main.rs           # Entry point (message loop, hotkey, window setup)
  lib.rs            # Library crate for testing without Windows deps
  app.rs            # Application state machine (1300+ lines)
  animation.rs      # Animation system with bezier easing
  log.rs            # Simple file-based logging (log! macro)
  layout/           # Layout engine for widget trees
  platform/
    win32/          # Windows-specific: DPI, events, rendering, icons, etc.
  theme/
    ast.rs          # Theme AST nodes
    lexer.rs        # Logos-based tokenizer
    theme.lalrpop   # LALRPOP grammar for .rasi files
    tree.rs         # ThemeTree - resolved theme values
    types.rs        # Color, Distance, Rect, etc.
  widget/           # Widget system (containers, listview, textbox, etc.)
```

## Code Style Guidelines

### Imports

Order imports as: std -> external crates -> crate modules
```rust
use std::path::Path;

use windows::Win32::Foundation::{HWND, LPARAM};

use crate::theme::tree::ThemeTree;
use crate::widget::Widget;
```

### Formatting

- Use `cargo fmt` before committing
- Default rustfmt settings (no custom configuration)
- 4-space indentation
- Max line width: 100 characters (soft limit)

### Types and Structs

- Use `#[derive(Clone, Copy, Debug)]` for simple value types
- Implement `Default` for configurable structs
- Use `f32` for graphics/UI values (Direct2D compatibility)
- Color values are 0.0-1.0 range (not 0-255)

```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}
```

### Naming Conventions

- Types: `PascalCase` (e.g., `ThemeTree`, `WidgetState`)
- Functions/methods: `snake_case` (e.g., `get_color`, `handle_event`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `TIMER_CURSOR_BLINK`)
- Modules: `snake_case` (e.g., `theme`, `win32`)
- Timer IDs, hotkey IDs: use `const` with descriptive names

### Error Handling

- Use `thiserror` for custom error types
- Return `Result<T, Error>` for fallible operations
- Use `windows::core::Error` for Win32 API errors
- Log errors with `log!()` macro before returning

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Invalid hex color: {0}")]
    InvalidHexColor(String),
    #[error("Unknown unit: {0}")]
    UnknownUnit(String),
}

// In functions:
if let Err(e) = some_operation() {
    log!("Operation failed: {:?}", e);
    return Err(e);
}
```

### Logging

Use the `log!()` macro for debug output (writes to `wolfy.log`):
```rust
log!("Creating window: {}x{}", width, height);
log!("Error: {:?}", error);
```

### Unsafe Code

- Minimize `unsafe` blocks - contain them in platform modules
- Document why `unsafe` is needed
- Validate inputs before unsafe operations

```rust
unsafe {
    // SAFETY: hwnd is valid, checked at creation
    UpdateLayeredWindow(hwnd, ...);
}
```

### Tests

- Place tests in `#[cfg(test)] mod tests` at the end of the file
- Use descriptive test names: `test_<what>_<condition>`
- Test edge cases and error conditions

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_hex_rgb() {
        let c = Color::from_hex("#ff0000").unwrap();
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.0);
    }

    #[test]
    fn test_color_invalid_hex() {
        assert!(Color::from_hex("not-a-color").is_err());
    }
}
```

### Widget Pattern

Widgets implement the `Widget` trait:
```rust
pub trait Widget {
    fn handle_event(&mut self, event: &Event, ctx: &LayoutContext) -> EventResult;
    fn render(&self, renderer: &mut Renderer, rect: Rect, ctx: &LayoutContext) -> Result<(), Error>;
    fn state(&self) -> WidgetState;
    fn set_state(&mut self, state: WidgetState);
    fn measure(&self, constraints: Constraints, ctx: &LayoutContext) -> MeasuredSize;
    fn arrange(&mut self, bounds: Rect, ctx: &LayoutContext);
}
```

### Theme Properties

Access theme values through `ThemeTree` with fallbacks:
```rust
let color = theme.get_color("textbox", state, "background-color", Color::BLACK);
let size = theme.get_number("window", None, "width", 928.0) as i32;
let family = theme.get_string("textbox", None, "font-family", "Segoe UI");
```

## Key Files to Understand

1. `src/app.rs` - Main application logic, message handling
2. `src/widget/mod.rs` - Widget trait and core types
3. `src/theme/tree.rs` - Theme value resolution
4. `src/platform/win32/render.rs` - Direct2D rendering
5. `default.rasi` - Theme file format example

## Build System Notes

- `build.rs` runs LALRPOP to generate parser from `theme.lalrpop`
- `build.rs` copies `default.rasi` to output directory
- Release builds use LTO and strip symbols for smaller binary
- The `lib.rs` exposes modules for testing without Windows dependencies
