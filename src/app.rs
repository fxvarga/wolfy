//! Application state machine for Wolfy

use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, SetTimer, WM_DPICHANGED, WM_PAINT, WM_TIMER,
};

use crate::animation::{Easing, WindowAnimator};
use crate::history::History;
use crate::log::{exe_dir, find_config_file};
use crate::mode::Mode;
use crate::platform::win32::{
    self, discover_all_apps, get_monitor_width, get_wallpaper_path, invalidate_window,
    reposition_window, resize_window, set_wallpaper, translate_message, Event, ImageLoader,
    MouseButton, PollingFileWatcher, Renderer, WindowConfig,
};
use crate::task_runner::{TaskRunner, TaskStatus};
use crate::tasks::{find_tasks_config, load_tasks_config, TaskItemState, TaskPanelPosition};
use crate::theme::tree::ThemeTree;
use crate::theme::types::{Color, ImageScale, LayoutContext, Rect};
use crate::widget::{
    ClockConfig, ClockPosition, CornerRadii, ElementData, ElementStyle, EventResult, GridItem,
    GridView, GridViewStyle, ListView, ListViewStyle, TailView, TailViewHit, TailViewStyle,
    TaskPanelState, TaskPanelStyle, Textbox, Widget, WidgetState, WidgetStyle,
};

/// Cursor blink timer ID
const TIMER_CURSOR_BLINK: usize = 1;
/// Cursor blink interval in milliseconds
const CURSOR_BLINK_MS: u32 = 530;
/// Theme file watcher timer ID
const TIMER_FILE_WATCH: usize = 2;
/// File watch check interval in milliseconds
const FILE_WATCH_MS: u32 = 500;
/// Animation timer ID
const TIMER_ANIMATION: usize = 3;
/// Animation frame interval in milliseconds (~60fps)
const ANIMATION_FRAME_MS: u32 = 16;
/// Clock update timer ID
const TIMER_CLOCK: usize = 4;
/// Clock update interval in milliseconds (1 second)
const CLOCK_UPDATE_MS: u32 = 1000;
/// Task poll timer ID
const TIMER_TASK_POLL: usize = 5;
/// Task poll interval in milliseconds (100ms for responsive status updates)
const TASK_POLL_MS: u32 = 100;
/// Tail view refresh timer ID
const TIMER_TAIL_REFRESH: usize = 6;
/// Tail view refresh interval in milliseconds
const TAIL_REFRESH_MS: u32 = 200;

/// Application version from Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Fuzzy match a query against a target string
/// Returns Some(score) if matched, None if not matched
/// Higher scores indicate better matches
fn fuzzy_match(target: &str, query: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    let target_chars: Vec<char> = target.chars().collect();
    let query_chars: Vec<char> = query.chars().collect();

    // Check if all query chars appear in order in target
    let mut target_idx = 0;
    let mut query_idx = 0;
    let mut score = 0i32;
    let mut consecutive = 0;
    let mut last_match_idx: Option<usize> = None;

    while query_idx < query_chars.len() && target_idx < target_chars.len() {
        if target_chars[target_idx] == query_chars[query_idx] {
            // Matched a character

            // Bonus for consecutive matches
            if let Some(last) = last_match_idx {
                if target_idx == last + 1 {
                    consecutive += 1;
                    score += consecutive * 5; // Growing bonus for consecutive
                } else {
                    consecutive = 0;
                }
            }

            // Bonus for matching at start
            if target_idx == 0 {
                score += 15;
            }

            // Bonus for matching after separator (space, -, _, etc)
            if target_idx > 0 {
                let prev = target_chars[target_idx - 1];
                if prev == ' ' || prev == '-' || prev == '_' || prev == '.' {
                    score += 10;
                }
            }

            // Bonus for matching uppercase in camelCase
            if target_chars[target_idx].is_uppercase() {
                score += 5;
            }

            last_match_idx = Some(target_idx);
            query_idx += 1;
            score += 1; // Base score for each match
        }
        target_idx += 1;
    }

    // All query chars must be found
    if query_idx == query_chars.len() {
        // Bonus for shorter targets (more precise match)
        score += (100 - target_chars.len().min(100) as i32);

        // Bonus if query matches a significant portion of target
        let coverage = (query_chars.len() * 100) / target_chars.len().max(1);
        score += coverage as i32 / 2;

        Some(score)
    } else {
        None
    }
}

/// Get the HyDE themes directory path
fn hyde_themes_dir() -> Option<PathBuf> {
    // First, try relative to exe: ../hyde/themes/ (for portable installs)
    let exe_relative = exe_dir().join("../hyde/themes");
    if exe_relative.is_dir() {
        log!("Found HyDE themes at exe-relative path: {:?}", exe_relative);
        return Some(exe_relative);
    }

    // Try standard HyDE location: ~/.config/hyde/themes/
    if let Some(home) = std::env::var_os("HOME") {
        let path = PathBuf::from(home).join(".config/hyde/themes");
        if path.is_dir() {
            return Some(path);
        }
    }
    // Windows fallback: %USERPROFILE%\.config\hyde\themes
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let path = PathBuf::from(profile).join(".config/hyde/themes");
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

/// Represents a HyDE theme with its metadata
#[derive(Clone, Debug)]
pub struct HydeTheme {
    /// Theme name (directory name)
    pub name: String,
    /// Full path to theme directory
    pub path: PathBuf,
    /// Path to first wallpaper (used as thumbnail)
    pub thumbnail: Option<PathBuf>,
}

/// Scan for HyDE themes in the themes directory
fn scan_hyde_themes() -> Vec<HydeTheme> {
    let Some(themes_dir) = hyde_themes_dir() else {
        log!("HyDE themes directory not found");
        return Vec::new();
    };

    log!("Scanning HyDE themes from: {:?}", themes_dir);

    let mut themes = Vec::new();

    let Ok(entries) = std::fs::read_dir(&themes_dir) else {
        log!("Failed to read themes directory");
        return Vec::new();
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        // Look for a thumbnail - prefer theme preview, fallback to first wallpaper
        let wallpapers_dir = path.join("wallpapers");
        let thumbnail = find_first_image(&wallpapers_dir);

        themes.push(HydeTheme {
            name: name.to_string(),
            path: path.clone(),
            thumbnail,
        });
    }

    // Sort alphabetically
    themes.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    log!("Found {} HyDE themes", themes.len());
    themes
}

/// Scan wallpapers for a specific theme
fn scan_theme_wallpapers(theme_name: &str) -> Vec<PathBuf> {
    let Some(themes_dir) = hyde_themes_dir() else {
        return Vec::new();
    };

    let wallpapers_dir = themes_dir.join(theme_name).join("wallpapers");
    if !wallpapers_dir.is_dir() {
        log!("Wallpapers directory not found for theme: {}", theme_name);
        return Vec::new();
    }

    log!("Scanning wallpapers from: {:?}", wallpapers_dir);

    let mut wallpapers = Vec::new();

    let Ok(entries) = std::fs::read_dir(&wallpapers_dir) else {
        return Vec::new();
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_image_file(&path) {
            wallpapers.push(path);
        }
    }

    // Sort by filename
    wallpapers.sort_by(|a, b| {
        let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        a_name.cmp(b_name)
    });

    log!(
        "Found {} wallpapers for theme: {}",
        wallpapers.len(),
        theme_name
    );
    wallpapers
}

/// Check if a path is an image file based on extension
fn is_image_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
    )
}

/// Find the first image file in a directory (sorted alphabetically)
fn find_first_image(dir: &Path) -> Option<PathBuf> {
    if !dir.is_dir() {
        return None;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return None;
    };

    let mut images: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_image_file(p))
        .collect();

    images.sort_by(|a, b| {
        let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
        a_name.cmp(b_name)
    });

    images.into_iter().next()
}

/// Child widget layout info
#[derive(Clone, Debug)]
pub struct ChildLayout {
    /// Widget name
    pub name: String,
    /// Calculated bounds
    pub bounds: Rect,
    /// Whether this widget expands
    pub expand: bool,
    /// Fixed width (if not expanding)
    pub fixed_width: Option<f32>,
}

/// Theme-derived layout settings
#[derive(Clone, Debug)]
pub struct ThemeLayout {
    /// Window border radius
    pub window_border_radius: f32,
    /// Window border color
    pub window_border_color: Color,
    /// Window background color (tint behind everything)
    pub window_background_color: Color,
    /// Mainbox padding
    pub mainbox_padding: f32,
    /// Mainbox children names (from theme)
    pub mainbox_children: Vec<String>,
    /// Wallpaper panel width
    pub wallpaper_panel_width: f32,
    /// Wallpaper panel background color (overlay)
    pub wallpaper_panel_bg: Color,
    /// Wallpaper panel corner radii (per-corner)
    pub wallpaper_panel_radii: CornerRadii,
    /// Wallpaper panel diagonal edge offset (0 = no diagonal, positive = slant from top-right)
    pub wallpaper_panel_diagonal: f32,
    /// Wallpaper panel fade width (how wide the feathered edge is along the diagonal)
    pub wallpaper_panel_fade_width: f32,
    /// Wallpaper panel fade color (color to fade TO at the diagonal edge)
    pub wallpaper_panel_fade_color: Option<Color>,
    /// Wallpaper panel fade opacity multiplier (0.0-1.0, controls how solid the fade is)
    pub wallpaper_panel_fade_opacity: f32,
    /// Listbox background color
    pub listbox_bg: Color,
    /// Listbox corner radii (per-corner)
    pub listbox_radii: CornerRadii,
    /// Listbox padding
    pub listbox_padding: f32,
    /// Listview padding
    pub listview_padding_top: f32,
    pub listview_padding_right: f32,
    pub listview_padding_bottom: f32,
    pub listview_padding_left: f32,
    /// Animation duration in milliseconds (0 = no animation)
    pub animation_duration_ms: u32,
    /// Animation easing type (ease-out, ease-in, ease-in-out, linear)
    pub animation_easing: String,
    /// Clock configuration for wallpaper panel overlay
    pub clock_config: ClockConfig,
}

impl ThemeLayout {
    /// Check if wallpaper panel should be shown
    pub fn show_wallpaper_panel(&self) -> bool {
        self.mainbox_children.iter().any(|c| c == "wallpaper-panel")
    }

    /// Check if a specific widget is in mainbox children
    pub fn has_child(&self, name: &str) -> bool {
        self.mainbox_children.iter().any(|c| c == name)
    }

    /// Get the fixed width for a widget (scaled)
    pub fn get_widget_width(&self, name: &str, scale: f32) -> Option<f32> {
        match name {
            "wallpaper-panel" => Some(self.wallpaper_panel_width * scale),
            _ => None, // Widget expands
        }
    }

    /// Check if a widget should expand
    pub fn widget_expands(&self, name: &str) -> bool {
        match name {
            "wallpaper-panel" => false, // Fixed width
            "listbox" => true,          // Expands to fill remaining space
            _ => true,                  // Default to expanding
        }
    }

    /// Calculate bounds for each child in mainbox (horizontal layout)
    pub fn calculate_mainbox_children_bounds(
        &self,
        content_x: f32,
        content_y: f32,
        content_width: f32,
        content_height: f32,
        scale: f32,
    ) -> Vec<ChildLayout> {
        let mut layouts = Vec::new();
        let mut used_width = 0.0;
        let mut expand_count = 0;

        // First pass: calculate fixed widths and count expanding widgets
        for name in &self.mainbox_children {
            let expand = self.widget_expands(name);
            let fixed_width = self.get_widget_width(name, scale);

            if !expand {
                if let Some(w) = fixed_width {
                    used_width += w;
                }
            } else {
                expand_count += 1;
            }

            layouts.push(ChildLayout {
                name: name.clone(),
                bounds: Rect::zero(), // Will be calculated in second pass
                expand,
                fixed_width,
            });
        }

        // Second pass: distribute remaining width to expanding widgets
        let remaining_width = content_width - used_width;
        let expand_width = if expand_count > 0 {
            remaining_width / expand_count as f32
        } else {
            0.0
        };

        let mut x = content_x;
        for layout in &mut layouts {
            let width = if layout.expand {
                expand_width
            } else {
                layout.fixed_width.unwrap_or(expand_width)
            };

            layout.bounds = Rect::new(x, content_y, width, content_height);
            x += width;
        }

        layouts
    }
}

impl Default for ThemeLayout {
    fn default() -> Self {
        Self {
            window_border_radius: 16.0,
            window_border_color: Color::from_hex("#f97e72").unwrap_or(Color::WHITE),
            window_background_color: Color::TRANSPARENT, // Default to no tint
            mainbox_padding: 20.0,
            mainbox_children: vec!["wallpaper-panel".to_string(), "listbox".to_string()],
            wallpaper_panel_width: 456.0,
            wallpaper_panel_bg: Color::from_hex("#262335e6").unwrap_or(Color::BLACK),
            wallpaper_panel_radii: CornerRadii::uniform(16.0),
            wallpaper_panel_diagonal: 0.0,     // No diagonal by default
            wallpaper_panel_fade_width: 100.0, // Fade gradient width along diagonal edge
            wallpaper_panel_fade_color: None,  // None = use listbox_bg color
            wallpaper_panel_fade_opacity: 0.7, // Default opacity multiplier (0.7 = softer fade)
            listbox_bg: Color::from_hex("#262335e6").unwrap_or(Color::BLACK),
            listbox_radii: CornerRadii::uniform(16.0),
            listbox_padding: 0.0,
            listview_padding_top: 16.0,
            listview_padding_right: 32.0,
            listview_padding_bottom: 16.0,
            listview_padding_left: 32.0,
            animation_duration_ms: 200,
            animation_easing: "ease-out-expo".to_string(),
            clock_config: ClockConfig::default(),
        }
    }
}

impl ThemeLayout {
    /// Load layout settings from theme
    pub fn from_theme(theme: &ThemeTree) -> Self {
        let default = Self::default();

        // Get mainbox children from theme (or use defaults)
        let mainbox_children = {
            let children = theme.get_children("mainbox");
            if children.is_empty() {
                default.mainbox_children.clone()
            } else {
                children
            }
        };

        // Helper to read corner radii from theme
        let read_corner_radii = |widget: &str, default_radii: CornerRadii| -> CornerRadii {
            let base =
                theme.get_number(widget, None, "border-radius", default_radii.top_left as f64)
                    as f32;
            CornerRadii {
                top_left: theme.get_number(widget, None, "border-top-left-radius", base as f64)
                    as f32,
                top_right: theme.get_number(widget, None, "border-top-right-radius", base as f64)
                    as f32,
                bottom_right: theme.get_number(
                    widget,
                    None,
                    "border-bottom-right-radius",
                    base as f64,
                ) as f32,
                bottom_left: theme.get_number(
                    widget,
                    None,
                    "border-bottom-left-radius",
                    base as f64,
                ) as f32,
            }
        };

        let layout = Self {
            window_border_radius: theme.get_number(
                "window",
                None,
                "border-radius",
                default.window_border_radius as f64,
            ) as f32,
            window_border_color: theme.get_color(
                "window",
                None,
                "border-color",
                default.window_border_color,
            ),
            window_background_color: theme.get_color(
                "window",
                None,
                "background-color",
                default.window_background_color,
            ),
            mainbox_padding: theme.get_number(
                "mainbox",
                None,
                "padding",
                default.mainbox_padding as f64,
            ) as f32,
            mainbox_children,
            wallpaper_panel_width: theme.get_number(
                "wallpaper-panel",
                None,
                "width",
                default.wallpaper_panel_width as f64,
            ) as f32,
            wallpaper_panel_bg: theme.get_color(
                "wallpaper-panel",
                None,
                "background-color",
                default.wallpaper_panel_bg,
            ),
            wallpaper_panel_radii: read_corner_radii(
                "wallpaper-panel",
                default.wallpaper_panel_radii,
            ),
            wallpaper_panel_diagonal: {
                let val = theme.get_number(
                    "wallpaper-panel",
                    None,
                    "diagonal-edge",
                    default.wallpaper_panel_diagonal as f64,
                ) as f32;
                log!("ThemeLayout: wallpaper_panel_diagonal = {}", val);
                val
            },
            wallpaper_panel_fade_width: {
                let val = theme.get_number(
                    "wallpaper-panel",
                    None,
                    "fade-width",
                    default.wallpaper_panel_fade_width as f64,
                ) as f32;
                log!("ThemeLayout: wallpaper_panel_fade_width = {}", val);
                val
            },
            wallpaper_panel_fade_color: theme.get_color_opt("wallpaper-panel", None, "fade-color"),
            wallpaper_panel_fade_opacity: theme.get_number(
                "wallpaper-panel",
                None,
                "fade-opacity",
                default.wallpaper_panel_fade_opacity as f64,
            ) as f32,
            listbox_bg: theme.get_color("listbox", None, "background-color", default.listbox_bg),
            listbox_radii: read_corner_radii("listbox", default.listbox_radii),
            listbox_padding: theme.get_number(
                "listbox",
                None,
                "padding-top",
                default.listbox_padding as f64,
            ) as f32,
            listview_padding_top: theme.get_number(
                "listview",
                None,
                "padding-top",
                default.listview_padding_top as f64,
            ) as f32,
            listview_padding_right: theme.get_number(
                "listview",
                None,
                "padding-right",
                default.listview_padding_right as f64,
            ) as f32,
            listview_padding_bottom: theme.get_number(
                "listview",
                None,
                "padding-bottom",
                default.listview_padding_bottom as f64,
            ) as f32,
            listview_padding_left: theme.get_number(
                "listview",
                None,
                "padding-left",
                default.listview_padding_left as f64,
            ) as f32,
            animation_duration_ms: theme.get_number(
                "window",
                None,
                "animation-duration",
                default.animation_duration_ms as f64,
            ) as u32,
            animation_easing: theme.get_string(
                "window",
                None,
                "animation-easing",
                &default.animation_easing,
            ),
            clock_config: ClockConfig {
                enabled: theme.get_bool(
                    "wallpaper-panel",
                    None,
                    "clock-enabled",
                    default.clock_config.enabled,
                ),
                position: ClockPosition::from_str(&theme.get_string(
                    "wallpaper-panel",
                    None,
                    "clock-position",
                    "top-right",
                )),
                time_format: theme.get_string(
                    "wallpaper-panel",
                    None,
                    "clock-format",
                    &default.clock_config.time_format,
                ),
                date_format: theme.get_string(
                    "wallpaper-panel",
                    None,
                    "clock-date-format",
                    &default.clock_config.date_format,
                ),
                font_family: theme.get_string(
                    "wallpaper-panel",
                    None,
                    "clock-font-family",
                    &default.clock_config.font_family,
                ),
                font_size: theme.get_number(
                    "wallpaper-panel",
                    None,
                    "clock-font-size",
                    default.clock_config.font_size as f64,
                ) as f32,
                date_font_size: theme.get_number(
                    "wallpaper-panel",
                    None,
                    "clock-date-font-size",
                    default.clock_config.date_font_size as f64,
                ) as f32,
                text_color: theme.get_color(
                    "wallpaper-panel",
                    None,
                    "clock-text-color",
                    default.clock_config.text_color,
                ),
                shadow_color: theme.get_color(
                    "wallpaper-panel",
                    None,
                    "clock-shadow-color",
                    default.clock_config.shadow_color,
                ),
                shadow_offset: (
                    theme.get_number(
                        "wallpaper-panel",
                        None,
                        "clock-shadow-offset-x",
                        default.clock_config.shadow_offset.0 as f64,
                    ) as f32,
                    theme.get_number(
                        "wallpaper-panel",
                        None,
                        "clock-shadow-offset-y",
                        default.clock_config.shadow_offset.1 as f64,
                    ) as f32,
                ),
                padding: theme.get_number(
                    "wallpaper-panel",
                    None,
                    "clock-padding",
                    default.clock_config.padding as f64,
                ) as f32,
            },
        };

        // Debug log the loaded theme layout
        log!(
            "ThemeLayout::from_theme - window_background_color: r={:.3} g={:.3} b={:.3} a={:.3}",
            layout.window_background_color.r,
            layout.window_background_color.g,
            layout.window_background_color.b,
            layout.window_background_color.a
        );
        log!(
            "ThemeLayout::from_theme - animation: {}ms, easing={}",
            layout.animation_duration_ms,
            layout.animation_easing
        );

        layout
    }
}

/// Load task panel style from theme
fn load_task_panel_style(theme: &ThemeTree) -> TaskPanelStyle {
    let default = TaskPanelStyle::default();

    let style = TaskPanelStyle {
        enabled: theme.get_bool("task-panel", None, "enabled", default.enabled),
        position: {
            let pos = theme.get_string("task-panel", None, "position", "left");
            match pos.to_lowercase().as_str() {
                "right" => TaskPanelPosition::Right,
                _ => TaskPanelPosition::Left,
            }
        },

        // Colors
        background_color: theme.get_color(
            "task-panel",
            None,
            "background-color",
            default.background_color,
        ),
        icon_color: theme.get_color("task-panel", None, "icon-color", default.icon_color),
        icon_color_hover: theme.get_color(
            "task-panel",
            None,
            "icon-color-hover",
            default.icon_color_hover,
        ),
        group_icon_color: theme.get_color(
            "task-panel",
            None,
            "group-icon-color",
            default.group_icon_color,
        ),
        text_color: theme.get_color("task-panel", None, "text-color", default.text_color),
        text_color_hover: theme.get_color(
            "task-panel",
            None,
            "text-color-hover",
            default.text_color_hover,
        ),
        item_background_color: theme.get_color(
            "task-panel",
            None,
            "item-background-color",
            default.item_background_color,
        ),
        selected_background_color: theme.get_color(
            "task-panel",
            None,
            "selected-background-color",
            default.selected_background_color,
        ),
        tree_line_color: theme.get_color(
            "task-panel",
            None,
            "tree-line-color",
            default.tree_line_color,
        ),
        chevron_color: theme.get_color("task-panel", None, "chevron-color", default.chevron_color),

        // Typography
        icon_font_family: theme.get_string(
            "task-panel",
            None,
            "icon-font-family",
            &default.icon_font_family,
        ),
        text_font_family: theme.get_string(
            "task-panel",
            None,
            "text-font-family",
            &default.text_font_family,
        ),
        icon_size: theme.get_number("task-panel", None, "icon-size", default.icon_size as f64)
            as f32,
        text_size: theme.get_number("task-panel", None, "text-size", default.text_size as f64)
            as f32,

        // Dimensions
        compact_width: theme.get_number(
            "task-panel",
            None,
            "compact-width",
            default.compact_width as f64,
        ) as f32,
        expanded_width: theme.get_number(
            "task-panel",
            None,
            "expanded-width",
            default.expanded_width as f64,
        ) as f32,
        item_height: theme.get_number(
            "task-panel",
            None,
            "item-height",
            default.item_height as f64,
        ) as f32,
        item_corner_radius: theme.get_number(
            "task-panel",
            None,
            "item-corner-radius",
            default.item_corner_radius as f64,
        ) as f32,
        padding: theme.get_number("task-panel", None, "padding", default.padding as f64) as f32,
        group_spacing: theme.get_number(
            "task-panel",
            None,
            "group-spacing",
            default.group_spacing as f64,
        ) as f32,
        item_spacing: theme.get_number(
            "task-panel",
            None,
            "item-spacing",
            default.item_spacing as f64,
        ) as f32,
        border_radius: theme.get_number(
            "task-panel",
            None,
            "border-radius",
            default.border_radius as f64,
        ) as f32,
        sub_item_indent: theme.get_number(
            "task-panel",
            None,
            "sub-item-indent",
            default.sub_item_indent as f64,
        ) as f32,

        // Icons
        chevron_collapsed: theme.get_string(
            "task-panel",
            None,
            "chevron-collapsed",
            &default.chevron_collapsed,
        ),
        chevron_expanded: theme.get_string(
            "task-panel",
            None,
            "chevron-expanded",
            &default.chevron_expanded,
        ),
        tree_branch: theme.get_string("task-panel", None, "tree-branch", &default.tree_branch),
        tree_corner: theme.get_string("task-panel", None, "tree-corner", &default.tree_corner),
    };

    log!(
        "Task panel style loaded: icon_font='{}', compact_width={}, expanded_width={}, enabled={}",
        style.icon_font_family,
        style.compact_width,
        style.expanded_width,
        style.enabled
    );

    style
}

/// Application state
pub struct App {
    hwnd: HWND,
    renderer: Renderer,
    config: WindowConfig,
    textbox: Textbox,
    listview: ListView,
    gridview: GridView,
    /// All available items (unfiltered)
    all_items: Vec<ElementData>,
    /// Usage history for sorting
    history: History,
    layout_ctx: LayoutContext,
    style: WidgetStyle,
    /// Theme-derived layout settings
    theme_layout: ThemeLayout,
    /// Cached background bitmap
    background_bitmap: Option<ID2D1Bitmap>,
    /// Path that was used to load the background bitmap (for cache invalidation)
    background_bitmap_path: Option<String>,
    /// File watcher for theme hot-reload
    theme_watcher: Option<PollingFileWatcher>,
    /// Window animator for fade effects
    animator: WindowAnimator,
    /// Track if window is visible (to avoid double-hide issues)
    is_visible: bool,
    /// Task panel state (for quick-launch shortcuts)
    task_panel: Option<TaskPanelState>,
    /// Task panel style (from theme)
    task_panel_style: TaskPanelStyle,
    /// Current operating mode (Launcher, ThemePicker, WallpaperPicker)
    current_mode: Mode,
    /// Currently selected HyDE theme (for WallpaperPicker to know which theme's wallpapers to show)
    current_theme: Option<String>,
    /// Background task runner for executing tasks without visible terminal
    task_runner: TaskRunner,
    /// Tail view widget for showing task output
    tailview: TailView,
}

impl App {
    /// Create new application
    pub fn new(hwnd: HWND, config: WindowConfig) -> Result<Self, windows::core::Error> {
        log!("App::new() starting, hwnd={:?}", hwnd);

        log!("  Creating Renderer...");
        let renderer = Renderer::new(hwnd)?;
        log!("  Renderer created");

        let dpi_info = renderer.dpi();
        log!(
            "  DPI info: dpi={}, scale={}",
            dpi_info.dpi,
            dpi_info.scale_factor
        );

        let layout_ctx = LayoutContext {
            dpi: dpi_info.dpi as f32,
            scale_factor: dpi_info.scale_factor,
            base_font_size: 16.0,
            parent_size: config.width as f32,
        };

        // Load theme from user config dir or exe directory
        let theme_path = find_config_file("default.rasi");
        log!("  Loading theme from {:?}", theme_path);
        let theme = match ThemeTree::load(&theme_path) {
            Ok(t) => {
                log!("  Theme loaded successfully");
                Some(t)
            }
            Err(e) => {
                log!("  Failed to load theme: {:?}, using defaults", e);
                None
            }
        };

        // Create textbox with theme style
        let style = theme
            .as_ref()
            .map(|t| WidgetStyle::from_theme_textbox(t, None))
            .unwrap_or_default();
        log!(
            "  Textbox style: font_size={}, font_family={}",
            style.font_size,
            style.font_family
        );

        let mut textbox = Textbox::new()
            .with_placeholder("Type to search...")
            .with_style(style.clone());
        textbox.set_state(WidgetState::Focused);

        // Load listview and element styles from theme
        let listview_style = theme
            .as_ref()
            .map(|t| ListViewStyle::from_theme(t, None))
            .unwrap_or_default();
        let element_style = theme
            .as_ref()
            .map(|t| ElementStyle::from_theme(t, None))
            .unwrap_or_default();

        let listview = ListView::new()
            .with_style(listview_style)
            .with_element_style(element_style);

        // Load gridview style from theme
        let gridview_style = theme
            .as_ref()
            .map(|t| GridViewStyle::from_theme(t, None))
            .unwrap_or_default();
        let gridview = GridView::new().with_style(gridview_style);

        // Load theme layout settings
        let theme_layout = theme
            .as_ref()
            .map(|t| ThemeLayout::from_theme(t))
            .unwrap_or_default();

        // Load usage history
        log!("  Loading usage history...");
        let history = History::load_default();

        // Discover installed applications
        log!("  Discovering installed applications...");
        let discovered_apps = discover_all_apps(Some(&history));
        log!("  Discovered {} applications", discovered_apps.len());

        // Convert AppEntry to ElementData
        // user_data = launch_target (what to execute)
        // We use launch_target for both launching and history tracking
        let all_items: Vec<ElementData> = discovered_apps
            .into_iter()
            .map(|app| {
                let mut elem = ElementData::new(&app.name, &app.launch_target);
                if !app.description.is_empty() {
                    elem = elem.with_subtext(&app.description);
                }
                if !app.icon_source.is_empty() {
                    elem = elem.with_icon(&app.icon_source);
                }
                elem
            })
            .collect();

        // Create theme file watcher for hot-reload
        log!("  Creating theme file watcher for: {:?}", theme_path);
        let theme_watcher = Some(PollingFileWatcher::new(&theme_path));

        // Create window animator from theme settings
        let easing = Easing::from_name(&theme_layout.animation_easing);
        let animator = WindowAnimator::new(theme_layout.animation_duration_ms, easing);
        log!(
            "  Created animator: {}ms, easing={:?}",
            theme_layout.animation_duration_ms,
            theme_layout.animation_easing
        );

        // Load task panel configuration
        let task_panel = if let Some(config_path) = find_tasks_config() {
            log!("  Loading tasks from {:?}", config_path);
            let config = load_tasks_config(&config_path);
            if config.groups.is_empty() {
                log!("  No task groups found, task panel disabled");
                None
            } else {
                log!("  Loaded {} task groups", config.groups.len());
                Some(TaskPanelState::new(config))
            }
        } else {
            log!("  No tasks.toml found, task panel disabled");
            None
        };

        // Load task panel style from theme (or use defaults)
        let task_panel_style = theme
            .as_ref()
            .map(|t| load_task_panel_style(t))
            .unwrap_or_default();

        // Initialize task runner and cleanup old log files
        let task_runner = TaskRunner::new();
        task_runner.cleanup_old_files();

        // Initialize tail view with theme styles
        let mut tailview = TailView::new();
        if let Some(ref t) = theme {
            tailview.set_style(TailViewStyle::from_theme(t, None));
            tailview.set_button_style(task_panel_style.clone());
        }

        log!("App::new() completed successfully");
        Ok(Self {
            hwnd,
            renderer,
            config,
            textbox,
            listview,
            gridview,
            all_items,
            history,
            layout_ctx,
            style,
            theme_layout,
            background_bitmap: None,
            background_bitmap_path: None,
            theme_watcher,
            animator,
            is_visible: false,
            task_panel,
            task_panel_style,
            current_mode: Mode::default(),
            current_theme: None,
            task_runner,
            tailview,
        })
    }

    /// Start cursor blink timer
    pub fn start_cursor_timer(&self) {
        unsafe {
            SetTimer(self.hwnd, TIMER_CURSOR_BLINK, CURSOR_BLINK_MS, None);
        }
    }

    /// Start file watch timer (for theme hot-reload)
    pub fn start_file_watch_timer(&self) {
        unsafe {
            SetTimer(self.hwnd, TIMER_FILE_WATCH, FILE_WATCH_MS, None);
        }
    }

    /// Stop file watch timer
    pub fn stop_file_watch_timer(&self) {
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_FILE_WATCH);
        }
    }

    /// Stop cursor blink timer
    pub fn stop_cursor_timer(&self) {
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_CURSOR_BLINK);
        }
    }

    /// Start animation timer
    fn start_animation_timer(&self) {
        unsafe {
            SetTimer(self.hwnd, TIMER_ANIMATION, ANIMATION_FRAME_MS, None);
        }
    }

    /// Stop animation timer
    fn stop_animation_timer(&self) {
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_ANIMATION);
        }
    }

    /// Start clock update timer (if clock is enabled)
    fn start_clock_timer(&self) {
        if self.theme_layout.clock_config.enabled {
            unsafe {
                SetTimer(self.hwnd, TIMER_CLOCK, CLOCK_UPDATE_MS, None);
            }
        }
    }

    /// Stop clock update timer
    fn stop_clock_timer(&self) {
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_CLOCK);
        }
    }

    /// Start task poll timer (for checking background task completion)
    fn start_task_poll_timer(&self) {
        unsafe {
            SetTimer(self.hwnd, TIMER_TASK_POLL, TASK_POLL_MS, None);
        }
    }

    /// Stop task poll timer
    fn stop_task_poll_timer(&self) {
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_TASK_POLL);
        }
    }

    /// Start tail refresh timer (for updating output view)
    fn start_tail_refresh_timer(&self) {
        log!("Starting tail refresh timer ({}ms interval), timer_id={}", TAIL_REFRESH_MS, TIMER_TAIL_REFRESH);
        unsafe {
            let result = SetTimer(self.hwnd, TIMER_TAIL_REFRESH, TAIL_REFRESH_MS, None);
            log!("SetTimer returned: {}", result);
        }
    }

    /// Stop tail refresh timer
    fn stop_tail_refresh_timer(&self) {
        log!("Stopping tail refresh timer");
        unsafe {
            let _ = KillTimer(self.hwnd, TIMER_TAIL_REFRESH);
        }
    }

    /// Handle window procedure messages
    pub fn handle_message(
        &mut self,
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        // Log ALL WM_TIMER events to debug
        if msg == WM_TIMER {
            log!(">>> WM_TIMER received: timer_id={}", wparam.0);
        }

        // Handle tail refresh timer FIRST (before translate_message)
        if msg == WM_TIMER && wparam.0 == TIMER_TAIL_REFRESH {
            log!(">>> TIMER_TAIL_REFRESH received, uses_tail_view={}", self.current_mode.uses_tail_view());
            if self.current_mode.uses_tail_view() {
                log!(">>> Calling tailview.refresh()");
                self.tailview.refresh();
                self.renderer.mark_dirty();
                invalidate_window(self.hwnd);
            }
            return Some(LRESULT(0));
        }

        // Log interesting messages
        if msg == 0x0100 || msg == 0x0101 || msg == 0x0102 || msg == 0x0010 || msg == 0x0112 {
            // WM_KEYDOWN, WM_KEYUP, WM_CHAR, WM_CLOSE, WM_SYSCOMMAND
            log!("handle_message: msg=0x{:04X}, wparam=0x{:X}", msg, wparam.0);
        }

        // Translate to our Event type
        if let Some(event) = translate_message(hwnd, msg, wparam, lparam) {
            log!("  Translated to event: {:?}", event);
            let result = self.handle_event(&event);
            log!(
                "  Result: consumed={}, needs_repaint={}, text_changed={}, submit={}, cancel={}",
                result.consumed,
                result.needs_repaint,
                result.text_changed,
                result.submit,
                result.cancel
            );

            if result.needs_repaint {
                // Mark renderer as dirty since content changed
                self.renderer.mark_dirty();
                invalidate_window(self.hwnd);
            }

            if result.submit {
                log!("  Calling on_submit()");
                self.on_submit();
            }

            if result.cancel {
                log!("  Calling on_cancel()");
                self.on_cancel();
            }

            if result.consumed {
                log!("  Returning consumed (LRESULT(0))");
                return Some(LRESULT(0));
            }
        }

        // Handle specific messages that need special treatment
        match msg {
            WM_PAINT => {
                self.paint();
                return Some(LRESULT(0));
            }
            WM_TIMER if wparam.0 == TIMER_CURSOR_BLINK => {
                self.textbox.toggle_cursor_blink();
                invalidate_window(self.hwnd);
                return Some(LRESULT(0));
            }
            WM_TIMER if wparam.0 == TIMER_FILE_WATCH => {
                self.check_theme_file_changed();
                return Some(LRESULT(0));
            }
            WM_TIMER if wparam.0 == TIMER_ANIMATION => {
                // Update animation state
                if self.animator.update() {
                    // Still animating - just update opacity without re-rendering content
                    let opacity = self.animator.get_opacity();
                    let _ = self.renderer.update_opacity_only(opacity);
                } else {
                    // Animation complete - stop the timer and start clock timer
                    self.stop_animation_timer();
                    self.start_clock_timer();
                }
                return Some(LRESULT(0));
            }
            WM_TIMER if wparam.0 == TIMER_CLOCK => {
                // Clock tick - repaint to update time display
                self.renderer.mark_dirty();
                invalidate_window(self.hwnd);
                return Some(LRESULT(0));
            }
            WM_TIMER if wparam.0 == TIMER_TASK_POLL => {
                // Poll background tasks for completion
                let status_changed = self.task_runner.poll();
                if status_changed {
                    // A task finished - repaint to update indicators
                    self.renderer.mark_dirty();
                    invalidate_window(self.hwnd);
                }
                return Some(LRESULT(0));
            }
            WM_DPICHANGED => {
                let new_dpi = (wparam.0 & 0xFFFF) as u32;
                let _ = self.handle_dpi_change(new_dpi);
                return Some(LRESULT(0));
            }
            _ => {}
        }

        None
    }

    /// Handle an event
    fn handle_event(&mut self, event: &Event) -> EventResult {
        use crate::platform::win32::event::KeyCode;

        // In tail view mode, handle events for the tail view
        if self.current_mode.uses_tail_view() {
            match event {
                Event::KeyDown { key, .. } => {
                    match *key {
                        KeyCode::Escape => {
                            // Back to launcher
                            self.exit_tail_view();
                            return EventResult::repaint();
                        }
                        KeyCode::Enter => {
                            // Activate selected button
                            match self.tailview.get_selected_button() {
                                TailViewHit::BackButton => {
                                    self.exit_tail_view();
                                }
                                TailViewHit::OpenTerminalButton => {
                                    self.open_terminal_and_close();
                                }
                                TailViewHit::RerunButton => {
                                    self.rerun_task();
                                }
                                TailViewHit::KillButton => {
                                    self.kill_current_task();
                                }
                                _ => {}
                            }
                            return EventResult::repaint();
                        }
                        KeyCode::Up => {
                            // Navigate to previous button
                            self.tailview.select_prev_button();
                            return EventResult::repaint();
                        }
                        KeyCode::Down => {
                            // Navigate to next button
                            self.tailview.select_next_button();
                            return EventResult::repaint();
                        }
                        KeyCode::Tab => {
                            // Tab also navigates buttons (forward)
                            self.tailview.select_next_button();
                            return EventResult::repaint();
                        }
                        KeyCode::Left => {
                            // Scroll up one line
                            self.tailview.scroll_by(-1);
                            return EventResult::repaint();
                        }
                        KeyCode::Right => {
                            // Scroll down one line
                            self.tailview.scroll_by(1);
                            return EventResult::repaint();
                        }
                        KeyCode::PageUp => {
                            self.tailview.page_up();
                            return EventResult::repaint();
                        }
                        KeyCode::PageDown => {
                            self.tailview.page_down();
                            return EventResult::repaint();
                        }
                        KeyCode::Home => {
                            self.tailview.scroll_to_top();
                            return EventResult::repaint();
                        }
                        KeyCode::End => {
                            self.tailview.scroll_to_bottom();
                            return EventResult::repaint();
                        }
                        _ => {}
                    }
                }
                Event::MouseWheel { delta, .. } => {
                    // Scroll content
                    if *delta > 0 {
                        self.tailview.scroll_by(-3);
                    } else {
                        self.tailview.scroll_by(3);
                    }
                    return EventResult::repaint();
                }
                Event::MouseDown { x, y, .. } => {
                    // Hit test buttons
                    match self.tailview.hit_test(*x as f32, *y as f32) {
                        TailViewHit::BackButton => {
                            self.exit_tail_view();
                            return EventResult::repaint();
                        }
                        TailViewHit::OpenTerminalButton => {
                            self.open_terminal_and_close();
                            return EventResult::repaint();
                        }
                        TailViewHit::RerunButton => {
                            self.rerun_task();
                            return EventResult::repaint();
                        }
                        TailViewHit::KillButton => {
                            self.kill_current_task();
                            return EventResult::repaint();
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            return EventResult::none();
        }

        // In grid modes, route navigation + Enter + mouse wheel to gridview.
        // Textbox still handles typing (for future filtering), but we don't use listview.
        if self.current_mode.uses_grid_view() {
            // Handle mouse wheel for grid scrolling
            if let Event::MouseWheel { .. } = event {
                let result = self.gridview.handle_event(event, &self.layout_ctx);
                if result.needs_repaint {
                    return result;
                }
            }

            if let Event::KeyDown { key, .. } = event {
                match *key {
                    KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Up
                    | KeyCode::Down
                    | KeyCode::Home
                    | KeyCode::End
                    | KeyCode::PageUp
                    | KeyCode::PageDown
                    | KeyCode::Enter => {
                        let mut result = self.gridview.handle_event(event, &self.layout_ctx);

                        // Handle grid submit based on current mode
                        if result.submit {
                            if let Some(item) = self.gridview.selected_item().cloned() {
                                log!(
                                    "Grid submit (mode={:?}): '{}' ({})",
                                    self.current_mode,
                                    item.title,
                                    item.user_data
                                );

                                match self.current_mode {
                                    Mode::ThemePicker => {
                                        // Set current theme and switch to WallpaperPicker
                                        self.current_theme = Some(item.user_data.clone());
                                        log!("Selected theme: {}", item.user_data);
                                        self.current_mode = Mode::WallpaperPicker;
                                        self.on_mode_changed();
                                        // Force repaint
                                        self.renderer.mark_dirty();
                                        invalidate_window(self.hwnd);
                                    }
                                    Mode::WallpaperPicker => {
                                        // Set wallpaper (user_data contains the full path)
                                        log!("Setting wallpaper: {}", item.user_data);
                                        self.set_wallpaper(&item.user_data);
                                        win32::hide_window(self.hwnd);
                                        self.is_visible = false;
                                    }
                                    Mode::Launcher | Mode::TailView => {
                                        // Should not happen, but handle gracefully
                                    }
                                }
                            }
                            result.submit = false;
                        }

                        if result.consumed {
                            return result;
                        }
                    }
                    _ => {}
                }
            }
        }

        // Handle mouse events for task panel and listview
        match event {
            Event::MouseMove { x, y } => {
                return self.handle_mouse_move(*x as f32, *y as f32);
            }
            Event::MouseDown {
                x,
                y,
                button: MouseButton::Left,
            } => {
                return self.handle_mouse_click(*x as f32, *y as f32);
            }
            Event::MouseWheel { delta, .. } => {
                // Route mouse wheel to listview in launcher mode (not grid view)
                // Scroll regardless of mouse position - the list is the main content
                if !self.current_mode.uses_grid_view() {
                    // Scroll: delta > 0 = scroll up, delta < 0 = scroll down
                    if *delta > 0 {
                        self.listview.scroll_by(-1);
                    } else if *delta < 0 {
                        self.listview.scroll_by(1);
                    }
                    return EventResult::repaint();
                }
            }
            _ => {}
        }

        // Handle focus lost - hide the launcher when clicking outside
        // Only process if window is currently visible (avoid double-hide)
        if matches!(event, Event::FocusLost) {
            if self.is_visible {
                log!("Focus lost while visible - hiding launcher");
                return EventResult {
                    needs_repaint: false,
                    consumed: true,
                    text_changed: false,
                    submit: false,
                    cancel: true, // This will trigger on_cancel() which hides the window
                };
            } else {
                log!("Focus lost while hidden - ignoring");
                return EventResult::none();
            }
        }

        // Check for navigation keys that should go to listview
        if let Event::KeyDown { key, .. } = event {
            match *key {
                // Tab toggles focus between task panel and list
                KeyCode::Tab => {
                    if let Some(ref mut task_panel) = self.task_panel {
                        if task_panel.focused {
                            // Move focus from task panel to list
                            task_panel.set_focus(false);
                            log!("Tab: focus moved to list");
                        } else if self.task_panel_style.enabled && task_panel.has_tasks() {
                            // Move focus from list to task panel
                            task_panel.set_focus(true);
                            log!("Tab: focus moved to task panel");
                        }
                        return EventResult {
                            needs_repaint: true,
                            consumed: true,
                            text_changed: false,
                            submit: false,
                            cancel: false,
                        };
                    }
                }
                // Arrow keys - route based on focus
                KeyCode::Down | KeyCode::Up => {
                    // Check if task panel has focus
                    let task_panel_focused = self
                        .task_panel
                        .as_ref()
                        .map(|tp| tp.focused)
                        .unwrap_or(false);

                    if task_panel_focused {
                        if let Some(ref mut task_panel) = self.task_panel {
                            if *key == KeyCode::Down {
                                task_panel.select_next();
                            } else {
                                task_panel.select_prev();
                            }
                            return EventResult {
                                needs_repaint: true,
                                consumed: true,
                                text_changed: false,
                                submit: false,
                                cancel: false,
                            };
                        }
                    } else {
                        // Route to listview
                        let result = self.listview.handle_event(event, &self.layout_ctx);
                        if result.consumed {
                            return result;
                        }
                    }
                }
                KeyCode::PageDown | KeyCode::PageUp => {
                    let result = self.listview.handle_event(event, &self.layout_ctx);
                    if result.consumed {
                        return result;
                    }
                }
                // Left/Right arrow keys - expand/collapse accordion when task panel focused
                KeyCode::Left | KeyCode::Right => {
                    let task_panel_focused = self
                        .task_panel
                        .as_ref()
                        .map(|tp| tp.focused)
                        .unwrap_or(false);

                    if task_panel_focused {
                        if let Some(ref mut task_panel) = self.task_panel {
                            if let Some(selected_idx) = task_panel.selected_item {
                                if let Some(item_state) =
                                    task_panel.item_states.get(selected_idx).cloned()
                                {
                                    if item_state.is_group_header {
                                        let group_idx = item_state.group_index;
                                        let is_expanded = task_panel
                                            .expanded_groups
                                            .get(group_idx)
                                            .copied()
                                            .unwrap_or(false);

                                        if *key == KeyCode::Right && !is_expanded {
                                            // Right opens accordion
                                            task_panel.toggle_group(group_idx);
                                            return EventResult {
                                                needs_repaint: true,
                                                consumed: true,
                                                text_changed: false,
                                                submit: false,
                                                cancel: false,
                                            };
                                        } else if *key == KeyCode::Left && is_expanded {
                                            // Left closes accordion
                                            task_panel.toggle_group(group_idx);
                                            return EventResult {
                                                needs_repaint: true,
                                                consumed: true,
                                                text_changed: false,
                                                submit: false,
                                                cancel: false,
                                            };
                                        }
                                    }
                                }
                            }
                        }
                        // Consume the event even if no toggle (don't let it go to textbox)
                        return EventResult {
                            needs_repaint: false,
                            consumed: true,
                            text_changed: false,
                            submit: false,
                            cancel: false,
                        };
                    }
                    // If not focused on task panel, let textbox handle Left/Right for cursor movement
                }
                // Enter activates selected item (task panel or list)
                KeyCode::Enter => {
                    // Check if task panel has focus and selection
                    let task_panel_focused = self
                        .task_panel
                        .as_ref()
                        .map(|tp| tp.focused && tp.selected_item.is_some())
                        .unwrap_or(false);

                    if task_panel_focused {
                        // Activate selected task panel item
                        // First, extract the task info to avoid borrow conflicts
                        let task_info: Option<(String, String, String)> =
                            if let Some(ref mut task_panel) = self.task_panel {
                                if let Some(selected_idx) = task_panel.selected_item {
                                    if let Some(item_state) =
                                        task_panel.item_states.get(selected_idx).cloned()
                                    {
                                        if item_state.is_group_header {
                                            // Toggle group
                                            task_panel.toggle_group(item_state.group_index);
                                            return EventResult {
                                                needs_repaint: true,
                                                consumed: true,
                                                text_changed: false,
                                                submit: false,
                                                cancel: false,
                                            };
                                        } else {
                                            // Get task info (group, name, script)
                                            if let Some(group) =
                                                task_panel.config.groups.get(item_state.group_index)
                                            {
                                                if let Some(task_idx) = item_state.task_index {
                                                    if let Some(task) = group.tasks.get(task_idx) {
                                                        log!(
                                                            "Task selected via keyboard: {} ({})",
                                                            task.name,
                                                            task.script
                                                        );
                                                        Some((
                                                            group.name.clone(),
                                                            task.name.clone(),
                                                            task.script.clone(),
                                                        ))
                                                    } else {
                                                        None
                                                    }
                                                } else {
                                                    None
                                                }
                                            } else {
                                                None
                                            }
                                        }
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                        // Now handle the task outside of the borrow
                        if let Some((group, name, script)) = task_info {
                            // Check if task is already running
                            if self.task_runner.is_running(&group, &name) {
                                // Enter tail view to show live output
                                log!("Task {}:{} is running, entering tail view", group, name);
                                self.enter_tail_view(&group, &name);
                            } else {
                                // Start task in background
                                log!("Starting background task: {}:{}", group, name);
                                if let Err(e) = self.task_runner.start_task(&group, &name, &script)
                                {
                                    log!("Failed to start task: {}", e);
                                }
                            }
                            return EventResult {
                                needs_repaint: true,
                                consumed: true,
                                text_changed: false,
                                submit: false,
                                cancel: false,
                            };
                        }
                    } else if self.listview.selected_data().is_some() {
                        return EventResult {
                            needs_repaint: false,
                            consumed: true,
                            text_changed: false,
                            submit: true,
                            cancel: false,
                        };
                    }
                }
                // F5 reloads theme
                KeyCode::F5 => {
                    log!("F5 pressed - reloading theme");
                    self.reload_theme();
                    return EventResult {
                        needs_repaint: true,
                        consumed: true,
                        text_changed: false,
                        submit: false,
                        cancel: false,
                    };
                }
                // F6 restarts the application
                KeyCode::F6 => {
                    log!("F6 pressed - restarting application");
                    self.restart_app();
                    return EventResult {
                        needs_repaint: false,
                        consumed: true,
                        text_changed: false,
                        submit: false,
                        cancel: false,
                    };
                }
                _ => {}
            }
        }

        // Forward other events to textbox
        let result = self.textbox.handle_event(event, &self.layout_ctx);

        // Handle text changes - filter the list
        if result.text_changed {
            self.on_text_changed();
        }

        // Reset cursor blink on any key event
        if matches!(event, Event::KeyDown { .. } | Event::Char(_)) {
            self.textbox.show_cursor();
        }

        result
    }

    /// Handle submit (Enter pressed)
    fn on_submit(&mut self) {
        // Get selected item from listview
        if let Some(data) = self.listview.selected_data() {
            let command = data.user_data.clone();
            let name = data.text.clone();
            log!("Launching: {} ({})", name, command);

            // Try to launch the application
            if let Err(e) = self.launch_app(&command) {
                log!("Failed to launch {}: {:?}", command, e);
            } else {
                // Record successful launch in history
                self.history.record_launch(&command);
            }
        }

        self.textbox.clear();
        win32::hide_window(self.hwnd);
        self.stop_cursor_timer();
    }

    /// Handle mouse move for task panel hover
    fn handle_mouse_move(&mut self, x: f32, y: f32) -> EventResult {
        if let Some(ref mut task_panel) = self.task_panel {
            let old_hovered = task_panel.hovered_item;
            task_panel.hovered_item = task_panel.hit_test(x, y);

            // Repaint if hover state changed
            if old_hovered != task_panel.hovered_item {
                return EventResult {
                    needs_repaint: true,
                    consumed: true,
                    text_changed: false,
                    submit: false,
                    cancel: false,
                };
            }
        }
        EventResult::none()
    }

    /// Handle mouse click for task panel and listview
    fn handle_mouse_click(&mut self, x: f32, y: f32) -> EventResult {
        // Check if click is in listview first (only for non-grid modes)
        if !self.current_mode.uses_grid_view() && self.listview.contains_point(x, y) {
            if let Some(idx) = self.listview.hit_test(x, y) {
                // Select and launch the clicked item
                self.listview.select(idx);
                log!("Listview item {} clicked", idx);
                return EventResult {
                    needs_repaint: true,
                    consumed: true,
                    text_changed: false,
                    submit: true,  // Trigger the item launch
                    cancel: false,
                };
            }
        }

        // Check if click is in task panel
        if let Some(ref mut task_panel) = self.task_panel {
            if let Some(item_idx) = task_panel.hit_test(x, y) {
                // Get item info before mutating
                let item_state = task_panel.item_states.get(item_idx).cloned();

                if let Some(state) = item_state {
                    if state.is_group_header {
                        // If the panel isn't focused yet, focus it first.
                        // This avoids toggling groups in compact mode (submenu isn't visible there).
                        if !task_panel.focused && self.task_panel_style.enabled {
                            task_panel.set_focus(true);
                            task_panel.selected_item = Some(item_idx);
                            log!(
                                "Task panel clicked (group header) while unfocused - focusing panel"
                            );
                        } else {
                            // Toggle group expansion only when focused/expanded
                            if task_panel.is_expanded() {
                                task_panel.toggle_group(state.group_index);
                                log!("Toggled group {} expansion", state.group_index);
                            }
                        }
                        return EventResult {
                            needs_repaint: true,
                            consumed: true,
                            text_changed: false,
                            submit: false,
                            cancel: false,
                        };
                    } else if let Some(task_idx) = state.task_index {
                        // If user clicks a task while panel isn't focused, focus it first.
                        if !task_panel.focused && self.task_panel_style.enabled {
                            task_panel.set_focus(true);
                            task_panel.selected_item = Some(item_idx);
                            log!("Task panel clicked (task) while unfocused - focusing panel");
                            return EventResult {
                                needs_repaint: true,
                                consumed: true,
                                text_changed: false,
                                submit: false,
                                cancel: false,
                            };
                        }

                        // Get task info
                        if let Some(group) = task_panel.config.groups.get(state.group_index) {
                            if let Some(task) = group.tasks.get(task_idx) {
                                let group_name = group.name.clone();
                                let task_name = task.name.clone();
                                let script = task.script.clone();
                                log!("Task clicked: {}:{} ({})", group_name, task_name, script);

                                // Check if task is already running
                                if self.task_runner.is_running(&group_name, &task_name) {
                                    // Enter tail view to show live output
                                    log!(
                                        "Task {}:{} is running, entering tail view",
                                        group_name,
                                        task_name
                                    );
                                    self.enter_tail_view(&group_name, &task_name);
                                } else {
                                    // Start task in background
                                    log!("Starting background task: {}:{}", group_name, task_name);
                                    if let Err(e) = self.task_runner.start_task(
                                        &group_name,
                                        &task_name,
                                        &script,
                                    ) {
                                        log!("Failed to start task: {}", e);
                                    }
                                }

                                return EventResult {
                                    needs_repaint: true,
                                    consumed: true,
                                    text_changed: false,
                                    submit: false,
                                    cancel: false,
                                };
                            }
                        }
                    }
                }
            }
        }
        EventResult::none()
    }

    /// Run a PowerShell script in a visible terminal window
    fn run_powershell_script(&self, script: &str) -> Result<(), windows::core::Error> {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        const CREATE_NEW_CONSOLE: u32 = 0x00000010;

        log!("Running task in terminal: {}", script);

        // Create a PowerShell script that:
        // 1. Hides the window immediately on start
        // 2. Sets console size
        // 3. Centers the window
        // 4. Shows the window
        // 5. Runs the user's script
        let center_and_run = format!(
            r#"Add-Type -Name W -Namespace N -MemberDefinition '[DllImport("user32.dll")]public static extern bool ShowWindow(IntPtr h,int c);[DllImport("user32.dll")]public static extern bool GetWindowRect(IntPtr h,out RECT r);[DllImport("user32.dll")]public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int h,bool r);[DllImport("kernel32.dll")]public static extern IntPtr GetConsoleWindow();public struct RECT{{public int L,T,R,B;}}'
Add-Type -AssemblyName System.Windows.Forms
$h=[N.W]::GetConsoleWindow()
[N.W]::ShowWindow($h,0)|Out-Null
$Host.UI.RawUI.WindowSize=New-Object System.Management.Automation.Host.Size(100,25)
$Host.UI.RawUI.BufferSize=New-Object System.Management.Automation.Host.Size(100,3000)
$r=New-Object N.W+RECT;[N.W]::GetWindowRect($h,[ref]$r)|Out-Null
$ww=$r.R-$r.L;$wh=$r.B-$r.T
$sw=[System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea.Width
$sh=[System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea.Height
[N.W]::MoveWindow($h,[int](($sw-$ww)/2),[int](($sh-$wh)/2),$ww,$wh,$false)|Out-Null
[N.W]::ShowWindow($h,1)|Out-Null
Clear-Host
{}"#,
            script
        );

        // Try Windows Terminal first (modern Windows 10/11)
        let result = Command::new("wt")
            .arg("new-tab")
            .arg("--")
            .arg("powershell")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-NoExit")
            .arg("-Command")
            .arg(&center_and_run)
            .spawn();

        if result.is_ok() {
            log!("Launched in Windows Terminal: {}", script);
            return Ok(());
        }

        // Fall back to starting PowerShell directly with a visible console
        log!("Windows Terminal not found, falling back to PowerShell directly");
        let result = Command::new("powershell")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-NoExit")
            .arg("-Command")
            .arg(&center_and_run)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn();

        match result {
            Ok(_) => {
                log!("Successfully ran PowerShell script: {}", script);
                Ok(())
            }
            Err(e) => {
                log!("Failed to run PowerShell script {}: {}", script, e);
                Err(windows::core::Error::from_win32())
            }
        }
    }

    /// Open a tail terminal for a running task to see live output
    fn open_task_tail(&self, group: &str, name: &str) -> Result<(), windows::core::Error> {
        use std::fs::File;
        use std::io::Write;

        let output_file = self.task_runner.get_output_file(group, name);
        let file_path = output_file.display().to_string();

        log!(
            "Opening tail terminal for task {}:{} ({})",
            group,
            name,
            file_path
        );

        // Build a simpler PowerShell script for tailing
        // Use escape sequences for quotes to avoid shell escaping issues
        let escaped_path = file_path.replace("'", "''"); // Escape single quotes for PS
        let tail_script = format!(
            r#"$Host.UI.RawUI.WindowTitle = 'Task: {} - {}'
Write-Host '=== Live output for task: {} ===' -ForegroundColor Cyan
Write-Host ''
Get-Content -Path '{}' -Wait -Tail 50"#,
            group.replace("'", "''"),
            name.replace("'", "''"),
            name.replace("'", "''"),
            escaped_path
        );

        // Write script to a temp file to avoid command line length and escaping issues
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join("wolfy_tail_script.ps1");

        if let Ok(mut file) = File::create(&script_path) {
            if let Err(e) = file.write_all(tail_script.as_bytes()) {
                log!("Failed to write temp script: {}", e);
                return Err(windows::core::Error::from_win32());
            }
        } else {
            log!("Failed to create temp script file");
            return Err(windows::core::Error::from_win32());
        }

        let script_path_str = script_path.display().to_string();
        log!("Created temp script at: {}", script_path_str);

        // Spawn the terminal in a separate thread to avoid re-entrancy issues
        // Windows Terminal's COM activation can send messages back to our window,
        // which would cause a RefCell borrow conflict
        std::thread::spawn(move || {
            use std::os::windows::process::CommandExt;
            use std::process::Command;

            const CREATE_NEW_CONSOLE: u32 = 0x00000010;

            // Try Windows Terminal with pwsh (PowerShell 7) first
            let result = Command::new("wt")
                .arg("new-tab")
                .arg("--")
                .arg("pwsh")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-NoExit")
                .arg("-File")
                .arg(&script_path_str)
                .spawn();

            if result.is_ok() {
                return;
            }

            // Try Windows Terminal with powershell (5.1)
            let result = Command::new("wt")
                .arg("new-tab")
                .arg("--")
                .arg("powershell")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-NoExit")
                .arg("-File")
                .arg(&script_path_str)
                .spawn();

            if result.is_ok() {
                return;
            }

            // Fall back to pwsh directly
            let result = Command::new("pwsh")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-NoExit")
                .arg("-File")
                .arg(&script_path_str)
                .creation_flags(CREATE_NEW_CONSOLE)
                .spawn();

            if result.is_ok() {
                return;
            }

            // Fall back to PowerShell 5.1 directly
            let _ = Command::new("powershell")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-NoExit")
                .arg("-File")
                .arg(&script_path_str)
                .creation_flags(CREATE_NEW_CONSOLE)
                .spawn();
        });

        log!("Spawned tail terminal in background thread");
        Ok(())
    }

    /// Enter tail view mode for a running task
    fn enter_tail_view(&mut self, group: &str, name: &str) {
        let output_file = self.task_runner.get_output_file(group, name);
        let task_key = format!("{}:{}", group, name);

        log!("Entering tail view for task: {} (file: {:?})", task_key, output_file);

        self.tailview.start_tail(task_key, output_file);
        self.current_mode = Mode::TailView;
        self.start_tail_refresh_timer();
        invalidate_window(self.hwnd);
    }

    /// Exit tail view mode and return to launcher
    fn exit_tail_view(&mut self) {
        log!("Exiting tail view, returning to launcher");

        self.tailview.stop_tail();
        self.stop_tail_refresh_timer();
        self.current_mode = Mode::Launcher;
        invalidate_window(self.hwnd);
    }

    /// Open external terminal for tail and close wolfy
    fn open_terminal_and_close(&mut self) {
        if let Some(task_key) = self.tailview.task_key().map(|s| s.to_string()) {
            let parts: Vec<&str> = task_key.split(':').collect();
            if parts.len() == 2 {
                let group = parts[0];
                let name = parts[1];

                log!("Opening external terminal for {} and closing", task_key);

                // Open external terminal
                let _ = self.open_task_tail(group, name);

                // Exit tail view and hide window
                self.tailview.stop_tail();
                self.stop_tail_refresh_timer();
                self.current_mode = Mode::Launcher;
                self.hide();
            }
        }
    }

    /// Rerun the current task (kill if running, clear output, restart)
    fn rerun_task(&mut self) {
        if let Some(task_key) = self.tailview.task_key().map(|s| s.to_string()) {
            let parts: Vec<&str> = task_key.split(':').collect();
            if parts.len() == 2 {
                let group = parts[0].to_string();
                let name = parts[1].to_string();

                log!("Rerunning task: {}", task_key);

                // Get the script from the task
                let script = if let Some(task) = self.task_runner.get_task(&group, &name) {
                    task.script.clone()
                } else {
                    // Task not found in runner, try to get it from tasks.toml
                    if let Some(config_path) = find_tasks_config() {
                        let config = load_tasks_config(&config_path);
                        if let Some(task_def) = config.find_task(&group, &name) {
                            task_def.script.clone()
                        } else {
                            log!("Cannot find script for task: {}", task_key);
                            return;
                        }
                    } else {
                        log!("Cannot find tasks.toml for task: {}", task_key);
                        return;
                    }
                };

                // Kill if running
                if self.task_runner.is_running(&group, &name) {
                    self.task_runner.kill_task(&group, &name);
                }

                // Clear the tailview
                self.tailview.stop_tail();

                // Restart the task
                if let Err(e) = self.task_runner.start_task(&group, &name, &script) {
                    log!("Failed to restart task: {}", e);
                    return;
                }

                // Re-enter tail view for the task
                let output_file = self.task_runner.get_output_file(&group, &name);
                self.tailview.start_tail(task_key, output_file);

                invalidate_window(self.hwnd);
            }
        }
    }

    /// Kill the current task being viewed in tailview
    fn kill_current_task(&mut self) {
        if let Some(task_key) = self.tailview.task_key().map(|s| s.to_string()) {
            let parts: Vec<&str> = task_key.split(':').collect();
            if parts.len() == 2 {
                let group = parts[0].to_string();
                let name = parts[1].to_string();

                log!("Killing task: {}", task_key);

                // Kill if running
                if self.task_runner.is_running(&group, &name) {
                    self.task_runner.kill_task(&group, &name);
                    log!("Task killed: {}", task_key);
                } else {
                    log!("Task not running: {}", task_key);
                }

                invalidate_window(self.hwnd);
            }
        }
    }

    /// Launch an application
    fn launch_app(&self, command: &str) -> Result<(), windows::core::Error> {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        // Try to run the command
        let result = Command::new("cmd")
            .args(["/C", "start", "", command])
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .spawn();

        match result {
            Ok(_) => {
                log!("Successfully spawned: {}", command);
                Ok(())
            }
            Err(e) => {
                log!("Failed to spawn {}: {}", command, e);
                Err(windows::core::Error::from_win32())
            }
        }
    }

    /// Restart the application by spawning a new instance and exiting
    fn restart_app(&self) {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        log!("Restarting application via cmd trampoline...");

        // Get the current executable path
        if let Ok(exe_path) = std::env::current_exe() {
            log!("Current exe: {:?}", exe_path);

            let exe_str = exe_path.to_string_lossy().to_string();
            log!("Exe path string: {}", exe_str);

            // Write a small batch file that waits and restarts
            // This avoids all escaping issues with cmd arguments
            let temp_dir = std::env::temp_dir();
            let batch_path = temp_dir.join("wolfy_restart.bat");
            let batch_content = format!(
                "@echo off\r\nping -n 2 localhost >nul\r\nstart \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
                exe_str
            );
            log!("Batch file: {:?}", batch_path);
            log!("Batch content: {}", batch_content);

            if let Err(e) = std::fs::write(&batch_path, &batch_content) {
                log!("Failed to write batch file: {}", e);
                return;
            }

            // Run the batch file
            match Command::new("cmd")
                .args(["/C", &batch_path.to_string_lossy()])
                .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
                .spawn()
            {
                Ok(_) => {
                    log!("Batch trampoline spawned, exiting current instance");
                }
                Err(e) => {
                    log!("Failed to spawn batch trampoline: {}", e);
                    return;
                }
            }

            // Exit immediately - batch will relaunch us in ~1 second
            unsafe {
                use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;
                PostQuitMessage(0);
            }
        } else {
            log!("Failed to get current exe path");
        }
    }

    /// Set the desktop wallpaper
    fn set_wallpaper(&self, path: &str) {
        use std::fs;

        log!("App::set_wallpaper() called with: {}", path);

        // Normalize the path - replace forward slashes and resolve ..
        // Don't use canonicalize() as it creates UNC paths on network shares
        let normalized = path
            .replace('/', "\\")
            .replace("\\.\\", "\\")
            .replace("\\..\\", "\\__PARENT__\\");

        // Simple parent resolution: repeatedly replace \dir\__PARENT__\ with \
        let mut result = normalized;
        loop {
            let before = result.clone();
            // Find pattern like \something\__PARENT__\ and remove both
            if let Some(parent_pos) = result.find("\\__PARENT__\\") {
                // Find the start of the directory before __PARENT__
                if let Some(dir_start) = result[..parent_pos].rfind('\\') {
                    result = format!("{}{}", &result[..dir_start], &result[parent_pos + 11..]);
                } else {
                    // No parent dir to remove, just remove __PARENT__
                    result = result.replace("\\__PARENT__", "");
                    break;
                }
            } else {
                break;
            }
            if before == result {
                break;
            }
        }

        log!("Normalized path: {}", result);

        // Check if path is on a network share (starts with \\ or contains network indicators)
        // If so, copy to a local temp file first to avoid SystemParametersInfoW issues
        let final_path = if result.starts_with("\\\\")
            || result.contains("\\Desktop\\Shared\\")
            || result.contains(":\\Users\\") && result.contains("\\Desktop\\Shared\\")
        {
            log!("Path appears to be on network share, copying to local temp...");

            // Get the file extension
            let extension = std::path::Path::new(&result)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("jpg");

            // Create temp path in user's temp directory
            let temp_dir = std::env::temp_dir();
            let temp_path = temp_dir.join(format!("wolfy_wallpaper.{}", extension));

            log!("Copying to: {:?}", temp_path);

            match fs::copy(&result, &temp_path) {
                Ok(bytes) => {
                    log!("Copied {} bytes to temp file", bytes);
                    temp_path.to_string_lossy().to_string()
                }
                Err(e) => {
                    log!("Failed to copy to temp: {:?}, trying original path", e);
                    result
                }
            }
        } else {
            result
        };

        log!("Final path for wallpaper: {}", final_path);

        if set_wallpaper(&final_path) {
            log!("Wallpaper set successfully");
        } else {
            log!("Failed to set wallpaper");
        }
    }

    /// Handle cancel (Escape pressed)
    fn on_cancel(&mut self) {
        // Avoid double-cancel if already hidden
        if !self.is_visible {
            log!("on_cancel() called but window already hidden - ignoring");
            return;
        }
        log!("on_cancel() called - hiding window");
        self.is_visible = false;
        self.textbox.clear();
        win32::hide_window(self.hwnd);
        self.stop_cursor_timer();
        log!("on_cancel() completed");
    }

    /// Handle text changes - filter the list with fuzzy matching
    fn on_text_changed(&mut self) {
        let query = self.textbox.text().to_lowercase();
        log!("on_text_changed() called, query='{}'", query);

        if query.is_empty() {
            // Show all items when no search query
            self.listview.set_items(self.all_items.clone());
        } else {
            // Filter and score items using fuzzy matching
            let mut scored: Vec<(i32, ElementData)> = self
                .all_items
                .iter()
                .filter_map(|item| {
                    let text_lower = item.text.to_lowercase();
                    let subtext_lower = item.subtext.as_ref().map(|s| s.to_lowercase());

                    // Try fuzzy match on text
                    if let Some(score) = fuzzy_match(&text_lower, &query) {
                        return Some((score, item.clone()));
                    }

                    // Try fuzzy match on subtext
                    if let Some(subtext) = &subtext_lower {
                        if let Some(score) = fuzzy_match(subtext, &query) {
                            // Subtext matches get lower priority
                            return Some((score - 100, item.clone()));
                        }
                    }

                    None
                })
                .collect();

            // Sort by score (higher is better)
            scored.sort_by(|a, b| b.0.cmp(&a.0));

            let filtered: Vec<ElementData> = scored.into_iter().map(|(_, item)| item).collect();
            self.listview.set_items(filtered);
        }
    }

    /// Reload theme from disk and apply all changes
    pub fn reload_theme(&mut self) {
        let theme_path = find_config_file("default.rasi");
        log!("reload_theme() - theme_path={:?}", theme_path);
        log!("reload_theme() - file exists={}", theme_path.exists());

        if !theme_path.exists() {
            log!("  Theme file does not exist, skipping reload");
            return;
        }

        let theme = match ThemeTree::load(&theme_path) {
            Ok(t) => {
                log!("  Theme reloaded successfully");
                t
            }
            Err(e) => {
                log!("  Failed to reload theme: {:?}", e);
                return;
            }
        };

        // Update textbox style
        self.style = WidgetStyle::from_theme_textbox(&theme, None);
        self.textbox.set_style(self.style.clone());
        log!(
            "  Updated textbox style: font_size={}, font_family={}",
            self.style.font_size,
            self.style.font_family
        );

        // Update listview and element styles
        let listview_style = ListViewStyle::from_theme(&theme, None);
        let element_style = ElementStyle::from_theme(&theme, None);
        self.listview.set_style(listview_style);
        self.listview.set_element_style(element_style);
        log!("  Updated listview styles");

        // Update gridview style
        let gridview_style = GridViewStyle::from_theme(&theme, None);
        self.gridview.set_style(gridview_style);
        log!("  Updated gridview style");

        // Update theme layout settings
        self.theme_layout = ThemeLayout::from_theme(&theme);
        log!(
            "  Updated theme layout: border_radius={}, wallpaper_width={}, mainbox_padding={}, children={:?}",
            self.theme_layout.window_border_radius,
            self.theme_layout.wallpaper_panel_width,
            self.theme_layout.mainbox_padding,
            self.theme_layout.mainbox_children
        );

        // Update task panel style
        self.task_panel_style = load_task_panel_style(&theme);
        log!(
            "  Updated task panel style: enabled={}, compact_width={}, expanded_width={}, icon_size={}",
            self.task_panel_style.enabled,
            self.task_panel_style.compact_width,
            self.task_panel_style.expanded_width,
            self.task_panel_style.icon_size
        );

        // Reload tasks.toml if it exists
        if let Some(tasks_path) = find_tasks_config() {
            let config = load_tasks_config(&tasks_path);
            if let Some(ref mut task_panel) = self.task_panel {
                // Preserve expanded state
                let old_expanded = task_panel.expanded_groups.clone();
                task_panel.config = config;
                // Restore expanded state for existing groups
                task_panel.expanded_groups = task_panel
                    .config
                    .groups
                    .iter()
                    .enumerate()
                    .map(|(i, _g)| old_expanded.get(i).copied().unwrap_or(false))
                    .collect();
                log!("  Reloaded tasks.toml");
            }
        }

        // Invalidate cached background bitmap to force reload
        self.background_bitmap = None;
        self.background_bitmap_path = None;
        log!("  Invalidated background bitmap cache");

        // Mark renderer as dirty
        self.renderer.mark_dirty();

        // Force repaint
        invalidate_window(self.hwnd);
        log!("reload_theme() completed");
    }

    /// Check file watcher and reload if theme changed
    fn check_theme_file_changed(&mut self) {
        if let Some(ref mut watcher) = self.theme_watcher {
            if watcher.check_modified() {
                log!("Theme file modified, reloading...");
                self.reload_theme();
            }
        }
    }

    /// Handle DPI change
    fn handle_dpi_change(&mut self, new_dpi: u32) -> Result<(), windows::core::Error> {
        self.renderer.handle_dpi_change(new_dpi)?;
        self.layout_ctx.dpi = new_dpi as f32;
        self.layout_ctx.scale_factor = new_dpi as f32 / 96.0;
        reposition_window(self.hwnd, &self.config);
        Ok(())
    }

    /// Paint the window - Dynamic layout based on theme's mainbox children
    ///
    /// Layout structure (driven by theme):
    /// - Window: fully transparent
    /// - Mainbox: rounded border with padding, contains children in order
    fn paint(&mut self) {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        log!("paint() called");

        if !self.renderer.begin_draw() {
            log!("  begin_draw() returned false, skipping paint");
            return;
        }

        // Clear with transparent (layered window)
        self.renderer.clear(Color::TRANSPARENT);

        // Get client size
        let (width, height) = win32::get_client_size(self.hwnd);
        log!("  Client size: {}x{}", width, height);
        let scale = self.layout_ctx.scale_factor;

        // Layout dimensions from theme
        let mainbox_padding = self.theme_layout.mainbox_padding * scale;
        let corner_radius = self.theme_layout.window_border_radius * scale;
        let border_width = 2.0 * scale;

        // Draw window background tint (if not transparent)
        // This fills the entire rounded window area with a tint color
        let window_bg = self.theme_layout.window_background_color;
        log!(
            "  Window background color: r={}, g={}, b={}, a={}",
            window_bg.r,
            window_bg.g,
            window_bg.b,
            window_bg.a
        );
        if window_bg.a > 0.0 {
            let window_bounds = D2D_RECT_F {
                left: mainbox_padding / 2.0,
                top: mainbox_padding / 2.0,
                right: width as f32 - mainbox_padding / 2.0,
                bottom: height as f32 - mainbox_padding / 2.0,
            };
            log!("  Drawing window background tint at {:?}", window_bounds);
            let _ = self.renderer.fill_rounded_rect(
                window_bounds,
                corner_radius,
                corner_radius,
                window_bg,
            );
        } else {
            log!("  Window background is transparent, skipping tint");
        }

        // Content area (inside mainbox padding)
        let content_x = mainbox_padding;
        let content_y = mainbox_padding;
        let content_width = width as f32 - mainbox_padding * 2.0;
        let content_height = height as f32 - mainbox_padding * 2.0;

        // In tail view mode, render only the tail view and skip other widgets
        if self.current_mode.uses_tail_view() {
            let tail_rect = Rect::new(content_x, content_y, content_width, content_height);
            let _ = self.tailview.render(&mut self.renderer, tail_rect, &self.layout_ctx);

            // Draw mainbox border
            let mainbox_bounds = D2D_RECT_F {
                left: mainbox_padding / 2.0,
                top: mainbox_padding / 2.0,
                right: width as f32 - mainbox_padding / 2.0,
                bottom: height as f32 - mainbox_padding / 2.0,
            };
            let _ = self.renderer.draw_rounded_rect(
                mainbox_bounds,
                corner_radius,
                corner_radius,
                self.theme_layout.window_border_color,
                border_width,
            );

            log!("  Calling end_draw() for tail view...");
            let opacity = self.animator.get_opacity();
            let result = self.renderer.end_draw_with_opacity(opacity);
            log!("  end_draw() result: {:?}, opacity: {}", result, opacity);
            return;
        }

        // Calculate layout for each mainbox child.
        // In picker modes we want a full-width listbox/grid, so we skip the wallpaper panel.
        let child_layouts = if self.current_mode.has_wallpaper_panel() {
            self.theme_layout.calculate_mainbox_children_bounds(
                content_x,
                content_y,
                content_width,
                content_height,
                scale,
            )
        } else {
            vec![ChildLayout {
                name: "listbox".to_string(),
                bounds: Rect::new(content_x, content_y, content_width, content_height),
                expand: true,
                fixed_width: None,
            }]
        };

        log!(
            "  Mainbox children: {:?}",
            child_layouts.iter().map(|c| &c.name).collect::<Vec<_>>()
        );

        // Track listbox bounds for rendering listview content later
        let mut listbox_bounds: Option<Rect> = None;

        // First pass: Draw backgrounds (in reverse z-order for proper layering)
        // Draw expanding/background widgets first (like listbox)
        for layout in &child_layouts {
            if layout.expand {
                self.draw_widget_background(&layout.name, &layout.bounds, corner_radius);
                if layout.name == "listbox" {
                    listbox_bounds = Some(layout.bounds.clone());
                }
            }
        }

        // Second pass: Draw fixed/foreground widgets (like wallpaper-panel) on top
        for layout in &child_layouts {
            if !layout.expand {
                self.draw_widget_background(&layout.name, &layout.bounds, corner_radius);
            }
        }

        // Render listview content inside listbox
        if let Some(listbox) = listbox_bounds {
            let listview_padding_top = self.theme_layout.listview_padding_top * scale;
            let listview_padding_bottom = self.theme_layout.listview_padding_bottom * scale;
            let listview_padding_left = self.theme_layout.listview_padding_left * scale;
            let listview_padding_right = self.theme_layout.listview_padding_right * scale;

            let listview_rect = Rect::new(
                listbox.x + listview_padding_left,
                listbox.y + listview_padding_top,
                listbox.width - listview_padding_left - listview_padding_right,
                listbox.height - listview_padding_top - listview_padding_bottom,
            );

            if listview_rect.height > 0.0 {
                // Keep widget bounds up-to-date for keyboard navigation/scroll logic
                if self.current_mode.uses_grid_view() {
                    self.gridview.arrange(listview_rect, &self.layout_ctx);
                    if !self.gridview.is_empty() {
                        log!("  Rendering gridview ({} items)...", self.gridview.len());
                        let _ = self.gridview.render(
                            &mut self.renderer,
                            listview_rect,
                            &self.layout_ctx,
                        );
                    }
                } else {
                    self.listview.arrange(listview_rect, &self.layout_ctx);
                    if !self.listview.is_empty() {
                        log!("  Rendering listview ({} items)...", self.listview.len());
                        let _ = self.listview.render(
                            &mut self.renderer,
                            listview_rect,
                            &self.layout_ctx,
                        );
                    }
                }
            }
        }

        // Draw mainbox border (coral rounded rect around everything)
        let mainbox_bounds = D2D_RECT_F {
            left: mainbox_padding / 2.0,
            top: mainbox_padding / 2.0,
            right: width as f32 - mainbox_padding / 2.0,
            bottom: height as f32 - mainbox_padding / 2.0,
        };
        let _ = self.renderer.draw_rounded_rect(
            mainbox_bounds,
            corner_radius,
            corner_radius,
            self.theme_layout.window_border_color,
            border_width,
        );

        // Draw version watermark in bottom right corner
        self.draw_version_watermark(width, height, mainbox_padding);

        log!("  Calling end_draw()...");
        let opacity = self.animator.get_opacity();
        let result = self.renderer.end_draw_with_opacity(opacity);
        log!("  end_draw() result: {:?}, opacity: {}", result, opacity);
    }

    /// Draw a widget's background based on its name
    fn draw_widget_background(&mut self, name: &str, bounds: &Rect, _window_corner_radius: f32) {
        let scale = self.layout_ctx.scale_factor;

        match name {
            "wallpaper-panel" => {
                // Extend width to overlap into right panel area (eliminates seam)
                let overlap = 8.0;
                let radii = self.theme_layout.wallpaper_panel_radii.scaled(scale);
                let diagonal = self.theme_layout.wallpaper_panel_diagonal * scale;
                self.draw_wallpaper_panel_clean(
                    bounds.x,
                    bounds.y,
                    bounds.width + overlap,
                    bounds.height,
                    radii,
                    diagonal,
                );
            }
            "listbox" => {
                let radii = self.theme_layout.listbox_radii.scaled(scale);
                let diagonal = if self.current_mode.has_wallpaper_panel() {
                    self.theme_layout.wallpaper_panel_diagonal * scale
                } else {
                    0.0
                };
                // Extend the listbox to the LEFT to fill the diagonal gap
                // The listbox needs to go underneath the wallpaper panel's diagonal edge
                self.draw_right_panel(
                    bounds.x - diagonal, // Extend left by the diagonal amount
                    bounds.y,
                    bounds.width + diagonal, // Add the diagonal to width to compensate
                    bounds.height,
                    self.theme_layout.listbox_bg,
                    radii,
                );
            }
            _ => {
                log!("  Unknown widget for background: {}", name);
            }
        }
    }

    /// Draw version watermark in bottom right corner
    fn draw_version_watermark(&mut self, width: i32, height: i32, padding: f32) {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        let version_text = format!("v{}", VERSION);
        let font_size = 10.0 * self.layout_ctx.scale_factor;

        // Create a small text format for the version
        let text_format = match self
            .renderer
            .create_text_format("Segoe UI", font_size, false, false)
        {
            Ok(fmt) => fmt,
            Err(_) => return,
        };

        // Position in bottom right, inside the mainbox padding
        let text_width = 60.0 * self.layout_ctx.scale_factor;
        let text_height = 14.0 * self.layout_ctx.scale_factor;
        let margin = 4.0 * self.layout_ctx.scale_factor;

        let rect = D2D_RECT_F {
            left: width as f32 - padding - text_width - margin,
            top: height as f32 - padding - text_height - margin,
            right: width as f32 - padding - margin,
            bottom: height as f32 - padding - margin,
        };

        // Draw with low opacity as a subtle watermark
        let watermark_color = Color::from_f32(1.0, 1.0, 1.0, 0.3);
        let _ = self.renderer.draw_text_right_aligned(
            &version_text,
            &text_format,
            rect,
            watermark_color,
        );
    }

    /// Draw the left wallpaper panel (no overlay - clean wallpaper)
    fn draw_wallpaper_panel_clean(
        &mut self,
        x: f32,
        y: f32,
        width: f32, // Already extended if needed by caller
        height: f32,
        radii: CornerRadii,
        diagonal: f32, // Diagonal edge offset (0 = no diagonal)
    ) {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        log!(
            "  draw_wallpaper_panel_clean: x={}, y={}, w={}, h={}, radii=({},{},{},{}), diagonal={}",
            x,
            y,
            width,
            height,
            radii.top_left,
            radii.top_right,
            radii.bottom_right,
            radii.bottom_left,
            diagonal,
        );

        let bounds = D2D_RECT_F {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };

        // Push a clip - use diagonal clip if diagonal > 0, otherwise regular rounded clip
        let _layer = if diagonal > 0.0 {
            self.renderer.push_diagonal_clip(bounds, radii, diagonal)
        } else {
            self.renderer.push_rounded_clip_corners(bounds, radii)
        };

        // Fallback dark background
        let fallback_bg = Color::from_f32(0.1, 0.1, 0.1, 1.0);
        let _ = self.renderer.fill_rect(bounds, fallback_bg);

        // Draw wallpaper image using cover mode to fill entire panel
        self.draw_background_image_in_rect(
            "auto".to_string(),
            ImageScale::Both, // Cover mode - ensures full width and height coverage
            x,
            y,
            width,
            height,
        );

        // Draw the fade gradient overlay along the diagonal edge
        // This creates a smooth feathered transition from wallpaper to listbox background
        let scale = self.layout_ctx.scale_factor;
        let fade_width = self.theme_layout.wallpaper_panel_fade_width * scale;

        if fade_width > 0.0 && diagonal > 0.0 {
            let fade_bounds = D2D_RECT_F {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            };

            // Use custom fade color if specified, otherwise fall back to listbox background
            let base_color = self
                .theme_layout
                .wallpaper_panel_fade_color
                .unwrap_or(self.theme_layout.listbox_bg);

            // Apply configurable opacity multiplier
            let fade_color = Color::from_f32(
                base_color.r,
                base_color.g,
                base_color.b,
                base_color.a * self.theme_layout.wallpaper_panel_fade_opacity,
            );

            let _ = self
                .renderer
                .fill_diagonal_fade(fade_bounds, fade_width, diagonal, fade_color);
        }

        // Pop the clip layer
        self.renderer.pop_layer();

        // Draw clock overlay if enabled
        if self.theme_layout.clock_config.enabled {
            self.draw_clock(x, y, width, height);
        }

        // Draw task panel overlay if enabled, has tasks, AND we're in Launcher mode
        if self.task_panel_style.enabled && self.current_mode.has_task_panel() {
            if let Some(ref mut task_panel) = self.task_panel {
                if task_panel.has_tasks() {
                    self.draw_task_panel(x, y, width, height);
                }
            }
        }
    }

    /// Draw the clock overlay on the wallpaper panel
    fn draw_clock(&mut self, panel_x: f32, panel_y: f32, panel_width: f32, panel_height: f32) {
        use chrono::Local;
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        let config = &self.theme_layout.clock_config;
        let scale = self.layout_ctx.scale_factor;

        // Get current time
        let now = Local::now();
        let time_str = now.format(&config.time_format).to_string();
        let date_str = if !config.date_format.is_empty() {
            Some(now.format(&config.date_format).to_string())
        } else {
            None
        };

        // Scale font sizes and offsets
        let time_font_size = config.font_size * scale;
        let date_font_size = config.date_font_size * scale;
        let padding = config.padding * scale;
        let shadow_offset_x = config.shadow_offset.0 * scale;
        let shadow_offset_y = config.shadow_offset.1 * scale;

        // Create text formats
        let time_format =
            match self
                .renderer
                .create_text_format(&config.font_family, time_font_size, true, false)
            {
                Ok(f) => f,
                Err(e) => {
                    log!("Failed to create time text format: {:?}", e);
                    return;
                }
            };

        // Measure time text
        let (time_width, time_height) =
            match self
                .renderer
                .measure_text(&time_str, &time_format, panel_width, panel_height)
            {
                Ok((w, h)) => (w, h),
                Err(e) => {
                    log!("Failed to measure time text: {:?}", e);
                    return;
                }
            };

        // Measure date text if present
        let (date_width, date_height, date_format) = if let Some(ref date) = date_str {
            let fmt = match self.renderer.create_text_format(
                &config.font_family,
                date_font_size,
                false,
                false,
            ) {
                Ok(f) => f,
                Err(e) => {
                    log!("Failed to create date text format: {:?}", e);
                    return;
                }
            };
            let (w, h) = match self
                .renderer
                .measure_text(date, &fmt, panel_width, panel_height)
            {
                Ok((w, h)) => (w, h),
                Err(e) => {
                    log!("Failed to measure date text: {:?}", e);
                    return;
                }
            };
            (w, h, Some(fmt))
        } else {
            (0.0, 0.0, None)
        };

        // Calculate total content size
        let total_width = time_width.max(date_width);
        let spacing = if date_str.is_some() { 4.0 * scale } else { 0.0 };
        let total_height = time_height + spacing + date_height;

        // Calculate position based on alignment
        let h_align = config.position.horizontal_align();
        let v_align = config.position.vertical_align();

        // Available space for positioning (panel minus padding)
        let avail_width = panel_width - 2.0 * padding;
        let avail_height = panel_height - 2.0 * padding;

        // Calculate top-left corner of the clock content block
        let content_x = panel_x + padding + (avail_width - total_width) * h_align;
        let content_y = panel_y + padding + (avail_height - total_height) * v_align;

        // Time text rect (centered horizontally within content block)
        let time_x = content_x + (total_width - time_width) / 2.0;
        let time_rect = D2D_RECT_F {
            left: time_x,
            top: content_y,
            right: time_x + time_width,
            bottom: content_y + time_height,
        };

        // Shadow rect (offset)
        let time_shadow_rect = D2D_RECT_F {
            left: time_rect.left + shadow_offset_x,
            top: time_rect.top + shadow_offset_y,
            right: time_rect.right + shadow_offset_x,
            bottom: time_rect.bottom + shadow_offset_y,
        };

        // Draw time shadow
        let _ = self.renderer.draw_text(
            &time_str,
            &time_format,
            time_shadow_rect,
            config.shadow_color,
        );

        // Draw time text
        let _ = self
            .renderer
            .draw_text(&time_str, &time_format, time_rect, config.text_color);

        // Draw date if present
        if let (Some(date), Some(ref date_fmt)) = (date_str, date_format) {
            let date_x = content_x + (total_width - date_width) / 2.0;
            let date_y = content_y + time_height + spacing;
            let date_rect = D2D_RECT_F {
                left: date_x,
                top: date_y,
                right: date_x + date_width,
                bottom: date_y + date_height,
            };

            // Shadow rect
            let date_shadow_rect = D2D_RECT_F {
                left: date_rect.left + shadow_offset_x,
                top: date_rect.top + shadow_offset_y,
                right: date_rect.right + shadow_offset_x,
                bottom: date_rect.bottom + shadow_offset_y,
            };

            // Draw date shadow
            let _ = self
                .renderer
                .draw_text(&date, date_fmt, date_shadow_rect, config.shadow_color);

            // Draw date text
            let _ = self
                .renderer
                .draw_text(&date, date_fmt, date_rect, config.text_color);
        }
    }

    /// Draw the task panel overlay on the wallpaper panel
    fn draw_task_panel(&mut self, panel_x: f32, panel_y: f32, panel_width: f32, panel_height: f32) {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        let scale = self.layout_ctx.scale_factor;
        let style = &self.task_panel_style;

        // Get task panel from self (we need to work with references carefully)
        let task_panel = match self.task_panel.as_mut() {
            Some(tp) => tp,
            None => return,
        };

        // Show expanded sidebar only when focused
        let show_expanded = task_panel.focused && task_panel.is_expanded();

        // Scale dimensions
        let outer_padding = style.padding * scale;
        let inner_padding = style.padding * scale;
        let panel_width_scaled = if show_expanded {
            style.expanded_width * scale
        } else {
            style.compact_width * scale
        };
        let panel_content_height = panel_height - 2.0 * outer_padding;
        let border_radius = style.border_radius * scale;

        // Row metrics
        let row_height = style.item_height * scale;
        let row_spacing = style.item_spacing * scale;
        let group_spacing = style.group_spacing * scale;
        let icon_size = style.icon_size * scale;
        let text_size = style.text_size * scale;
        let sub_indent = style.sub_item_indent * scale;
        let item_radius = style.item_corner_radius * scale;

        // Calculate panel position based on style.position
        let panel_left = match style.position {
            TaskPanelPosition::Left => panel_x + outer_padding,
            TaskPanelPosition::Right => panel_x + panel_width - panel_width_scaled - outer_padding,
        };
        let panel_top = panel_y + outer_padding;

        // Store panel bounds for hit-testing
        task_panel.panel_bounds = Rect::new(
            panel_left,
            panel_top,
            panel_width_scaled,
            panel_content_height,
        );

        // Draw panel background
        let bg_rect = D2D_RECT_F {
            left: panel_left,
            top: panel_top,
            right: panel_left + panel_width_scaled,
            bottom: panel_top + panel_content_height,
        };
        let _ = self.renderer.fill_rounded_rect(
            bg_rect,
            border_radius,
            border_radius,
            style.background_color,
        );

        // Create text formats
        let icon_format =
            match self
                .renderer
                .create_text_format(&style.icon_font_family, icon_size, false, false)
            {
                Ok(f) => f,
                Err(e) => {
                    log!("Failed to create task panel icon format: {:?}", e);
                    return;
                }
            };

        let text_format =
            match self
                .renderer
                .create_text_format(&style.text_font_family, text_size, false, false)
            {
                Ok(f) => f,
                Err(e) => {
                    log!("Failed to create task panel text format: {:?}", e);
                    return;
                }
            };

        // Clear previous item states
        task_panel.item_states.clear();

        // Clone data needed to avoid borrow conflicts
        let groups: Vec<_> = task_panel.config.groups.iter().cloned().collect();
        let expanded_groups: Vec<_> = task_panel.expanded_groups.clone();
        let hovered = task_panel.hovered_item;
        let selected = task_panel.selected_item;
        let is_focused = task_panel.focused;

        // Get task statuses for indicator dots
        let task_statuses = self.task_runner.get_all_statuses();

        // Calculate pulse animation for running tasks (using time)
        let pulse_opacity = {
            use std::time::{SystemTime, UNIX_EPOCH};
            let millis = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            // Pulse between 0.4 and 1.0 over ~1 second cycle
            let t = (millis % 1000) as f32 / 1000.0;
            let pulse = (t * std::f32::consts::PI * 2.0).sin() * 0.5 + 0.5;
            0.4 + pulse * 0.6
        };

        let mut y = panel_top + inner_padding;

        // Compact mode: group icons only
        if !show_expanded {
            for (group_idx, group) in groups.iter().enumerate() {
                let item_index = task_panel.item_states.len();
                let is_hovered = hovered == Some(item_index);
                let is_selected = is_focused && selected == Some(item_index);

                // Background behind icon
                let row_rect = D2D_RECT_F {
                    left: panel_left + inner_padding,
                    top: y,
                    right: panel_left + panel_width_scaled - inner_padding,
                    bottom: y + row_height,
                };

                let bg = if is_selected {
                    style.selected_background_color
                } else if is_hovered {
                    style.item_background_color
                } else {
                    Color::TRANSPARENT
                };
                if bg.a > 0.0 {
                    let _ = self
                        .renderer
                        .fill_rounded_rect(row_rect, item_radius, item_radius, bg);
                }

                let icon_color = if is_hovered || is_selected {
                    style.icon_color_hover
                } else {
                    style.group_icon_color
                };
                // In compact mode, shift icon left a few pixels to visually center it in the hover rect
                // This compensates for font rendering and visual perception
                let icon_offset = 3.0 * scale;
                let icon_rect = D2D_RECT_F {
                    left: row_rect.left - icon_offset,
                    top: row_rect.top,
                    right: row_rect.right - icon_offset,
                    bottom: row_rect.bottom,
                };
                let _ = self.renderer.draw_text_centered(
                    &group.icon,
                    &icon_format,
                    icon_rect,
                    icon_color,
                );

                task_panel.item_states.push(TaskItemState {
                    group_index: group_idx,
                    task_index: None,
                    bounds: Rect::new(
                        row_rect.left,
                        row_rect.top,
                        row_rect.right - row_rect.left,
                        row_height,
                    ),
                    is_group_header: true,
                });

                y += row_height + group_spacing;
            }

            // Tooltip only in compact mode; collect first to avoid borrow conflicts
            let tooltip_data: Option<(String, f32, f32)> =
                if let Some(hovered_idx) = task_panel.hovered_item {
                    if let Some((group, task)) = task_panel.get_task_at_index(hovered_idx) {
                        if let Some(item_state) = task_panel.item_states.get(hovered_idx) {
                            let tooltip_text = if let Some(t) = task {
                                t.name.clone()
                            } else {
                                group.name.clone()
                            };
                            Some((
                                tooltip_text,
                                item_state.bounds.x + item_state.bounds.width + 6.0 * scale,
                                item_state.bounds.y,
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

            // Release task_panel borrow before drawing tooltip
            let _ = task_panel;
            if let Some((text, x, y)) = tooltip_data {
                self.draw_tooltip(&text, x, y, scale);
            }

            return;
        }

        // Expanded mode
        for (group_idx, group) in groups.iter().enumerate() {
            let has_tasks = !group.tasks.is_empty();
            let is_group_expanded = expanded_groups.get(group_idx).copied().unwrap_or(false);

            // Group row
            let item_index = task_panel.item_states.len();
            let is_hovered = hovered == Some(item_index);
            let is_selected = is_focused && selected == Some(item_index);

            let row_rect = D2D_RECT_F {
                left: panel_left + inner_padding,
                top: y,
                right: panel_left + panel_width_scaled - inner_padding,
                bottom: y + row_height,
            };

            let bg = if is_selected {
                style.selected_background_color
            } else if is_hovered {
                style.item_background_color
            } else {
                Color::TRANSPARENT
            };
            if bg.a > 0.0 {
                let _ = self
                    .renderer
                    .fill_rounded_rect(row_rect, item_radius, item_radius, bg);
            }

            // Icon
            let icon_box = row_height;
            let icon_rect = D2D_RECT_F {
                left: row_rect.left,
                top: row_rect.top,
                right: row_rect.left + icon_box,
                bottom: row_rect.bottom,
            };

            let group_icon_color = if is_hovered || is_selected {
                style.icon_color_hover
            } else {
                style.group_icon_color
            };
            let _ = self.renderer.draw_text_centered(
                &group.icon,
                &icon_format,
                icon_rect,
                group_icon_color,
            );

            // Chevron
            let chevron_rect = D2D_RECT_F {
                left: row_rect.right - icon_box,
                top: row_rect.top,
                right: row_rect.right,
                bottom: row_rect.bottom,
            };

            if has_tasks {
                let chevron = if is_group_expanded {
                    &style.chevron_expanded
                } else {
                    &style.chevron_collapsed
                };
                let _ = self.renderer.draw_text_centered(
                    chevron,
                    &icon_format,
                    chevron_rect,
                    style.chevron_color,
                );
            }

            // Group label
            let label_rect = D2D_RECT_F {
                left: icon_rect.right + 6.0 * scale,
                top: row_rect.top,
                right: if has_tasks {
                    chevron_rect.left - 6.0 * scale
                } else {
                    row_rect.right - 6.0 * scale
                },
                bottom: row_rect.bottom,
            };
            let text_color = if is_hovered || is_selected {
                style.text_color_hover
            } else {
                style.text_color
            };
            let _ = self
                .renderer
                .draw_text(&group.name, &text_format, label_rect, text_color);

            // Hit-test bounds for group row
            task_panel.item_states.push(TaskItemState {
                group_index: group_idx,
                task_index: None,
                bounds: Rect::new(
                    row_rect.left,
                    row_rect.top,
                    row_rect.right - row_rect.left,
                    row_height,
                ),
                is_group_header: true,
            });

            y += row_height + row_spacing;

            // Sub-items only when focused/expanded and group is open
            if has_tasks && is_group_expanded {
                for (task_idx, task) in group.tasks.iter().enumerate() {
                    let item_index = task_panel.item_states.len();
                    let is_hovered = hovered == Some(item_index);
                    let is_selected = is_focused && selected == Some(item_index);

                    let task_row = D2D_RECT_F {
                        left: row_rect.left,
                        top: y,
                        right: row_rect.right,
                        bottom: y + row_height,
                    };

                    let bg = if is_selected {
                        style.selected_background_color
                    } else if is_hovered {
                        style.item_background_color
                    } else {
                        Color::TRANSPARENT
                    };
                    if bg.a > 0.0 {
                        let _ =
                            self.renderer
                                .fill_rounded_rect(task_row, item_radius, item_radius, bg);
                    }

                    // Tree prefix
                    let is_last = task_idx + 1 == group.tasks.len();
                    let prefix = if is_last {
                        &style.tree_corner
                    } else {
                        &style.tree_branch
                    };

                    let prefix_rect = D2D_RECT_F {
                        left: task_row.left + sub_indent,
                        top: task_row.top,
                        right: task_row.left + sub_indent + 24.0 * scale,
                        bottom: task_row.bottom,
                    };
                    let _ = self.renderer.draw_text(
                        prefix,
                        &text_format,
                        prefix_rect,
                        style.tree_line_color,
                    );

                    // Task label
                    let label_rect = D2D_RECT_F {
                        left: prefix_rect.right + 6.0 * scale,
                        top: task_row.top,
                        right: task_row.right - 6.0 * scale,
                        bottom: task_row.bottom,
                    };
                    let text_color = if is_hovered || is_selected {
                        style.text_color_hover
                    } else {
                        style.text_color
                    };
                    let _ =
                        self.renderer
                            .draw_text(&task.name, &text_format, label_rect, text_color);

                    // Status indicator dot (if task has run status)
                    let task_key = format!("{}:{}", group.name, task.name);
                    if let Some(status) = task_statuses.get(&task_key) {
                        // Choose color based on status
                        let (dot_color, should_pulse) = match status {
                            TaskStatus::Running => (
                                Color::from_f32(0.3, 0.6, 1.0, pulse_opacity), // Blue, pulsing
                                true,
                            ),
                            TaskStatus::Completed => (
                                Color::from_f32(0.3, 0.8, 0.3, 1.0), // Green
                                false,
                            ),
                            TaskStatus::Failed => (
                                Color::from_f32(0.9, 0.3, 0.3, 1.0), // Red
                                false,
                            ),
                        };

                        // Draw the dot at the right edge of the row
                        let dot_size = 8.0 * scale;
                        let dot_x = task_row.right - dot_size - 8.0 * scale;
                        let dot_y = (task_row.top + task_row.bottom) / 2.0;

                        // Draw filled circle
                        let _ = self.renderer.fill_ellipse(
                            dot_x,
                            dot_y,
                            dot_size / 2.0,
                            dot_size / 2.0,
                            dot_color,
                        );

                        // Force repaint if pulsing (to animate)
                        if should_pulse {
                            invalidate_window(self.hwnd);
                        }
                    }

                    // Hit-test bounds for task row
                    task_panel.item_states.push(TaskItemState {
                        group_index: group_idx,
                        task_index: Some(task_idx),
                        bounds: Rect::new(
                            task_row.left,
                            task_row.top,
                            task_row.right - task_row.left,
                            row_height,
                        ),
                        is_group_header: false,
                    });

                    y += row_height + row_spacing;
                }
            }

            y += group_spacing;
        }

        // Apply any pending selection now that item_states is populated
        task_panel.apply_pending_selection();
    }

    /// Draw a tooltip near the given position
    fn draw_tooltip(&mut self, text: &str, x: f32, y: f32, scale: f32) {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        let font_size = 12.0 * scale;
        let padding = 8.0 * scale; // Increased padding for better appearance
        let bg_color = Color::from_f32(0.1, 0.1, 0.1, 0.95);
        let text_color = Color::WHITE;

        let text_format = match self.renderer.create_text_format(
            &self.task_panel_style.text_font_family,
            font_size,
            false,
            false,
        ) {
            Ok(f) => f,
            Err(_) => return,
        };

        // Measure text - use large max width to prevent wrapping
        let (text_width, text_height) =
            match self
                .renderer
                .measure_text(text, &text_format, 500.0 * scale, 50.0 * scale)
            {
                Ok((w, h)) => (w, h),
                Err(_) => return,
            };

        // Draw background
        let bg_rect = D2D_RECT_F {
            left: x,
            top: y,
            right: x + text_width + 2.0 * padding,
            bottom: y + text_height + 2.0 * padding,
        };
        let _ = self
            .renderer
            .fill_rounded_rect(bg_rect, 4.0 * scale, 4.0 * scale, bg_color);

        // Draw text
        let text_rect = D2D_RECT_F {
            left: x + padding,
            top: y + padding,
            right: x + padding + text_width,
            bottom: y + padding + text_height,
        };
        let _ = self
            .renderer
            .draw_text(text, &text_format, text_rect, text_color);
    }

    /// Draw the right panel with solid color and per-corner radii
    fn draw_right_panel(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: crate::theme::types::Color,
        radii: CornerRadii,
    ) {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        log!(
            "  draw_right_panel: x={}, y={}, w={}, h={}, color=({},{},{},{}), radii=({},{},{},{})",
            x,
            y,
            width,
            height,
            color.r,
            color.g,
            color.b,
            color.a,
            radii.top_left,
            radii.top_right,
            radii.bottom_right,
            radii.bottom_left,
        );

        let bounds = D2D_RECT_F {
            left: x,
            top: y,
            right: x + width,
            bottom: y + height,
        };

        // Use per-corner radii
        let _ = self
            .renderer
            .fill_rounded_rect_corners(bounds, radii, color);
    }

    /// Draw the background image in a specific rectangle, loading and caching as needed
    fn draw_background_image_in_rect(
        &mut self,
        path: String,
        scale: ImageScale,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        log!(
            "  draw_background_image_in_rect: path={}, x={}, y={}, w={}, h={}",
            path,
            x,
            y,
            width,
            height
        );

        // Resolve "auto" to wallpaper path
        log!("  calling get_wallpaper_path...");
        let resolved_path = if path.eq_ignore_ascii_case("auto") {
            match get_wallpaper_path() {
                Some(p) => {
                    let path_str = p.to_string_lossy().into_owned();
                    log!("  got wallpaper path: {}", path_str);
                    path_str
                }
                None => {
                    log!("  No wallpaper found for 'auto' path");
                    return;
                }
            }
        } else {
            path.clone()
        };

        // Check if we need to reload the bitmap
        let need_reload = match &self.background_bitmap_path {
            Some(cached_path) => cached_path != &resolved_path,
            None => true,
        };

        if need_reload {
            log!("  Loading background image: {}", resolved_path);

            // Load the image
            let loader = match ImageLoader::new() {
                Ok(l) => l,
                Err(e) => {
                    log!("  Failed to create ImageLoader: {:?}", e);
                    return;
                }
            };

            // For the wallpaper panel, load with height scaling to ensure vertical coverage
            let loaded = match scale {
                ImageScale::None => loader.load_from_file(Path::new(&resolved_path)),
                ImageScale::Width => loader.load_scaled(
                    Path::new(&resolved_path),
                    width as u32,
                    0,
                    ImageScale::Width,
                ),
                ImageScale::Height => loader.load_scaled(
                    Path::new(&resolved_path),
                    0,
                    height as u32,
                    ImageScale::Height,
                ),
                ImageScale::Both => {
                    loader.load_cover(Path::new(&resolved_path), width as u32, height as u32)
                }
            };

            let loaded = match loaded {
                Ok(l) => l,
                Err(e) => {
                    log!("  Failed to load image: {:?}", e);
                    return;
                }
            };

            log!("  Loaded image: {}x{}", loaded.width(), loaded.height());

            // Create D2D bitmap
            let bitmap = match self.renderer.create_bitmap(&loaded) {
                Ok(b) => b,
                Err(e) => {
                    log!("  Failed to create D2D bitmap: {:?}", e);
                    return;
                }
            };

            self.background_bitmap = Some(bitmap);
            self.background_bitmap_path = Some(resolved_path);
        }

        // Draw the cached bitmap in the specified rect
        if let Some(ref bitmap) = self.background_bitmap {
            let bounds = windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            };
            let _ = self.renderer.draw_bitmap_cover(bitmap, bounds, 1.0);
        }
    }

    /// Get the window handle
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Get current input text
    pub fn input_text(&self) -> &str {
        self.textbox.text()
    }

    /// Get current mode
    pub fn current_mode(&self) -> Mode {
        self.current_mode
    }

    /// Show the window in a specific mode
    ///
    /// - If window is hidden, show it in the requested mode
    /// - If window is visible in the SAME mode, hide it (toggle behavior)
    /// - If window is visible in a DIFFERENT mode, switch to the new mode
    pub fn show_mode(&mut self, mode: Mode) {
        log!(
            "show_mode({:?}) called, is_visible={}, current_mode={:?}",
            mode,
            self.is_visible,
            self.current_mode
        );

        if self.is_visible {
            if self.current_mode == mode {
                // Same mode hotkey pressed while visible = toggle off
                log!("  Same mode, hiding window");
                self.hide();
                return;
            } else {
                // Different mode hotkey = switch modes
                log!("  Switching from {:?} to {:?}", self.current_mode, mode);
                self.current_mode = mode;
                self.on_mode_changed();
                // Force repaint for the new mode
                self.renderer.mark_dirty();
                invalidate_window(self.hwnd);
                return;
            }
        }

        // Window is hidden, show it in the requested mode
        self.current_mode = mode;
        self.show();
    }

    /// Resize window based on current mode
    /// Returns true if size actually changed
    fn resize_for_mode(&mut self) -> bool {
        let (target_width, target_height) = match self.current_mode {
            Mode::Launcher | Mode::TailView => (self.config.width, self.config.height),
            Mode::ThemePicker | Mode::WallpaperPicker => {
                let monitor_width = get_monitor_width();
                let grid_width = (monitor_width - 60).max(800); // 30px padding on each side
                let grid_height = 520; // Taller for grid view
                (grid_width, grid_height)
            }
        };

        // Check current window size to avoid unnecessary resize
        let current_size = self.renderer.get_size();
        let dpi_scale = self.renderer.dpi().scale_factor;
        let scaled_target_width = (target_width as f32 * dpi_scale) as i32;
        let scaled_target_height = (target_height as f32 * dpi_scale) as i32;

        if current_size.0 == scaled_target_width && current_size.1 == scaled_target_height {
            log!(
                "resize_for_mode: already at target size {}x{}, skipping",
                target_width,
                target_height
            );
            return false;
        }

        log!(
            "resize_for_mode: resizing from {:?} to {}x{}",
            current_size,
            target_width,
            target_height
        );

        match self.current_mode {
            Mode::Launcher | Mode::TailView => {
                win32::reposition_window(self.hwnd, &self.config);
            }
            Mode::ThemePicker | Mode::WallpaperPicker => {
                resize_window(self.hwnd, target_width, target_height, 0.4);
            }
        }

        // Force renderer to recreate buffers at new size
        let _ = self.renderer.handle_resize();

        // Clear cached bitmap since size changed
        self.background_bitmap = None;
        self.background_bitmap_path = None;

        true
    }

    /// Called when mode changes while window is visible
    fn on_mode_changed(&mut self) {
        log!("on_mode_changed() to {:?}", self.current_mode);

        // Hide window temporarily to avoid flash during resize
        let needs_resize = {
            let (target_width, target_height) = match self.current_mode {
                Mode::Launcher | Mode::TailView => (self.config.width, self.config.height),
                Mode::ThemePicker | Mode::WallpaperPicker => {
                    let monitor_width = get_monitor_width();
                    ((monitor_width - 60).max(800), 520)
                }
            };
            let current_size = self.renderer.get_size();
            let dpi_scale = self.renderer.dpi().scale_factor;
            let scaled_w = (target_width as f32 * dpi_scale) as i32;
            let scaled_h = (target_height as f32 * dpi_scale) as i32;
            current_size.0 != scaled_w || current_size.1 != scaled_h
        };

        if needs_resize {
            // Hide, resize, then show to avoid flash
            win32::hide_window(self.hwnd);
            self.resize_for_mode();
            win32::show_window(self.hwnd);
        }

        // Setup content for the mode
        self.setup_mode_content();
    }

    /// Setup content for the current mode (without resizing)
    fn setup_mode_content(&mut self) {
        match self.current_mode {
            Mode::Launcher => {
                // Reset textbox and show all apps
                self.textbox.clear();
                self.listview.set_items(self.all_items.clone());
                self.textbox.set_state(WidgetState::Focused);
            }
            Mode::ThemePicker => {
                log!("  ThemePicker mode - scanning HyDE themes");
                self.textbox.clear();
                self.listview.set_items(vec![]);

                let themes = scan_hyde_themes();
                let items: Vec<GridItem> = themes
                    .into_iter()
                    .map(|theme| {
                        let mut item = GridItem::new(&theme.name, &theme.name);
                        if let Some(thumb) = theme.thumbnail {
                            item = item.with_image(thumb.to_string_lossy().to_string());
                        }
                        item
                    })
                    .collect();

                log!("  Loaded {} themes into grid", items.len());
                self.gridview.set_items(items);
            }
            Mode::WallpaperPicker => {
                log!("  WallpaperPicker mode - scanning wallpapers");
                self.textbox.clear();
                self.listview.set_items(vec![]);

                // Use current_theme if set, otherwise show nothing or a message
                let items: Vec<GridItem> = if let Some(ref theme_name) = self.current_theme {
                    let wallpapers = scan_theme_wallpapers(theme_name);
                    wallpapers
                        .into_iter()
                        .map(|wp| {
                            let filename = wp
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Wallpaper");
                            // user_data = full path for setting wallpaper
                            GridItem::new(filename, wp.to_string_lossy().to_string())
                                .with_image(wp.to_string_lossy().to_string())
                        })
                        .collect()
                } else {
                    log!("  No theme selected - wallpaper grid will be empty");
                    Vec::new()
                };

                log!("  Loaded {} wallpapers into grid", items.len());
                self.gridview.set_items(items);
            }
            Mode::TailView => {
                // TailView is set up via enter_tail_view(), not here
                // Just keep the textbox unfocused
                self.textbox.set_state(WidgetState::Normal);
            }
        }

        // Reset task panel focus when switching modes
        if let Some(ref mut task_panel) = self.task_panel {
            task_panel.set_focus(false);
        }
    }

    /// Hide the window
    pub fn hide(&mut self) {
        if !self.is_visible {
            log!("hide() called but window already hidden - ignoring");
            return;
        }
        log!("hide() - hiding window");
        self.is_visible = false;
        self.textbox.clear();
        win32::hide_window(self.hwnd);
        self.stop_cursor_timer();
        self.stop_animation_timer();
        self.stop_clock_timer();
        self.stop_task_poll_timer();
        self.animator.clear();
        log!("hide() completed");
    }

    /// Show the window (internal helper)
    pub fn show(&mut self) {
        log!("show() - showing window in {:?} mode", self.current_mode);
        self.is_visible = true;

        // Resize window based on mode (this also updates renderer buffers)
        self.resize_for_mode();

        // Actually show the window
        win32::show_window(self.hwnd);

        // Setup based on mode (don't call resize_for_mode again)
        self.setup_mode_content();

        self.textbox.show_cursor();
        self.start_cursor_timer();
        self.start_task_poll_timer();

        // Mark renderer as dirty to force a full render
        self.renderer.mark_dirty();

        // Start fade-in animation
        self.animator.start_fade_in();
        self.start_animation_timer();
        log!("  Started fade-in animation");

        // Force an immediate paint
        self.paint();
        log!("show() completed");
    }

    /// Toggle window visibility (called from hotkey) - legacy method
    /// Now delegates to show_mode with the current mode
    pub fn toggle_visibility(&mut self) {
        log!("toggle_visibility() called, delegating to show_mode");
        self.show_mode(self.current_mode);
    }
}
