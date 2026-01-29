//! Application state machine for Wolfy

use std::path::Path;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Direct2D::ID2D1Bitmap;
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, SetTimer, WM_DPICHANGED, WM_PAINT, WM_TIMER,
};

use crate::animation::{Easing, WindowAnimator};
use crate::history::History;
use crate::log::exe_dir;
use crate::platform::win32::{
    self, discover_all_apps, get_wallpaper_path, invalidate_window, reposition_window,
    translate_message, Event, ImageLoader, MouseButton, PollingFileWatcher, Renderer, WindowConfig,
};
use crate::tasks::{find_tasks_config, load_tasks_config, TaskItemState, TaskPanelPosition};
use crate::theme::tree::ThemeTree;
use crate::theme::types::{Color, ImageScale, LayoutContext, Rect};
use crate::widget::{
    ClockConfig, ClockPosition, CornerRadii, ElementData, ElementStyle, EventResult, ListView,
    ListViewStyle, TaskPanelState, TaskPanelStyle, Textbox, Widget, WidgetState, WidgetStyle,
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
            wallpaper_panel_diagonal: theme.get_number(
                "wallpaper-panel",
                None,
                "diagonal-edge",
                default.wallpaper_panel_diagonal as f64,
            ) as f32,
            wallpaper_panel_fade_width: theme.get_number(
                "wallpaper-panel",
                None,
                "fade-width",
                default.wallpaper_panel_fade_width as f64,
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

    TaskPanelStyle {
        enabled: theme.get_bool("task-panel", None, "enabled", default.enabled),
        position: {
            let pos = theme.get_string("task-panel", None, "position", "left");
            match pos.to_lowercase().as_str() {
                "right" => TaskPanelPosition::Right,
                _ => TaskPanelPosition::Left,
            }
        },
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
        font_family: theme.get_string("task-panel", None, "font-family", &default.font_family),
        icon_size: theme.get_number("task-panel", None, "icon-size", default.icon_size as f64)
            as f32,
        width: theme.get_number("task-panel", None, "width", default.width as f64) as f32,
        padding: theme.get_number("task-panel", None, "padding", default.padding as f64) as f32,
        group_spacing: theme.get_number(
            "task-panel",
            None,
            "group-spacing",
            default.group_spacing as f64,
        ) as f32,
        task_spacing: theme.get_number(
            "task-panel",
            None,
            "task-spacing",
            default.task_spacing as f64,
        ) as f32,
        border_radius: theme.get_number(
            "task-panel",
            None,
            "border-radius",
            default.border_radius as f64,
        ) as f32,
    }
}

/// Application state
pub struct App {
    hwnd: HWND,
    renderer: Renderer,
    config: WindowConfig,
    textbox: Textbox,
    listview: ListView,
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

        // Load theme from exe directory
        let theme_path = exe_dir().join("default.rasi");
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

        log!("App::new() completed successfully");
        Ok(Self {
            hwnd,
            renderer,
            config,
            textbox,
            listview,
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

    /// Handle window procedure messages
    pub fn handle_message(
        &mut self,
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
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

        // Handle mouse events for task panel
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
                        // First, extract the script to run (if any) to avoid borrow conflicts
                        let script_to_run: Option<String> =
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
                                            // Get script to run
                                            if let Some(group) =
                                                task_panel.config.groups.get(item_state.group_index)
                                            {
                                                if let Some(task_idx) = item_state.task_index {
                                                    if let Some(task) = group.tasks.get(task_idx) {
                                                        log!(
                                                            "Running task via keyboard: {} ({})",
                                                            task.name,
                                                            task.script
                                                        );
                                                        Some(task.script.clone())
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

                        // Now run the script outside of the borrow
                        if let Some(script) = script_to_run {
                            self.run_powershell_script(&script);
                            win32::hide_window(self.hwnd);
                            return EventResult {
                                needs_repaint: false,
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

    /// Handle mouse click for task panel
    fn handle_mouse_click(&mut self, x: f32, y: f32) -> EventResult {
        // Check if click is in task panel
        if let Some(ref mut task_panel) = self.task_panel {
            if let Some(item_idx) = task_panel.hit_test(x, y) {
                // Get item info before mutating
                let item_state = task_panel.item_states.get(item_idx).cloned();

                if let Some(state) = item_state {
                    if state.is_group_header {
                        // Toggle group expansion
                        task_panel.toggle_group(state.group_index);
                        log!("Toggled group {} expansion", state.group_index);
                        return EventResult {
                            needs_repaint: true,
                            consumed: true,
                            text_changed: false,
                            submit: false,
                            cancel: false,
                        };
                    } else if let Some(task_idx) = state.task_index {
                        // Run the task
                        if let Some(group) = task_panel.config.groups.get(state.group_index) {
                            if let Some(task) = group.tasks.get(task_idx) {
                                let script = task.script.clone();
                                let name = task.name.clone();
                                log!("Running task: {} ({})", name, script);

                                // Run the script
                                if let Err(e) = self.run_powershell_script(&script) {
                                    log!("Failed to run task {}: {:?}", name, e);
                                }

                                // Hide launcher after running task
                                self.textbox.clear();
                                win32::hide_window(self.hwnd);
                                self.stop_cursor_timer();

                                return EventResult {
                                    needs_repaint: false,
                                    consumed: true,
                                    text_changed: false,
                                    submit: false,
                                    cancel: true,
                                };
                            }
                        }
                    }
                }
            }
        }
        EventResult::none()
    }

    /// Run a PowerShell script
    fn run_powershell_script(&self, script: &str) -> Result<(), windows::core::Error> {
        use std::os::windows::process::CommandExt;
        use std::process::Command;

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        // Check if it's a file path or inline command
        let result = if script.ends_with(".ps1") {
            // Run as script file
            Command::new("powershell")
                .args([
                    "-ExecutionPolicy",
                    "Bypass",
                    "-WindowStyle",
                    "Hidden",
                    "-File",
                    script,
                ])
                .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
                .spawn()
        } else {
            // Run as inline command
            Command::new("powershell")
                .args([
                    "-ExecutionPolicy",
                    "Bypass",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    script,
                ])
                .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
                .spawn()
        };

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
        let theme_path = exe_dir().join("default.rasi");
        log!("reload_theme() - exe_dir={:?}", exe_dir());
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
            "  Updated task panel style: enabled={}, width={}, icon_size={}",
            self.task_panel_style.enabled,
            self.task_panel_style.width,
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
                    .map(|(i, g)| old_expanded.get(i).copied().unwrap_or(g.expanded))
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

        // Calculate layout for each mainbox child
        let child_layouts = self.theme_layout.calculate_mainbox_children_bounds(
            content_x,
            content_y,
            content_width,
            content_height,
            scale,
        );

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

            if listview_rect.height > 0.0 && !self.listview.is_empty() {
                log!("  Rendering listview ({} items)...", self.listview.len());
                let _ = self
                    .listview
                    .render(&mut self.renderer, listview_rect, &self.layout_ctx);
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
                let diagonal = self.theme_layout.wallpaper_panel_diagonal * scale;
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

            // Use a semi-transparent version of the listbox color for a softer shadow effect
            // This creates a gentle vignette rather than a hard transition
            let fade_color = Color::from_f32(
                self.theme_layout.listbox_bg.r,
                self.theme_layout.listbox_bg.g,
                self.theme_layout.listbox_bg.b,
                self.theme_layout.listbox_bg.a * 0.7, // Reduce opacity for softer effect
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

        // Draw task panel overlay if enabled and has tasks
        if self.task_panel_style.enabled {
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

        // Scale dimensions
        let panel_width_scaled = style.width * scale;
        let padding = style.padding * scale;
        let icon_size = style.icon_size * scale;
        let group_spacing = style.group_spacing * scale;
        let task_spacing = style.task_spacing * scale;
        let border_radius = style.border_radius * scale;

        // Calculate panel position based on style.position
        let panel_left = match style.position {
            TaskPanelPosition::Left => panel_x + padding,
            TaskPanelPosition::Right => panel_x + panel_width - panel_width_scaled - padding,
        };
        let panel_top = panel_y + padding;
        let panel_content_height = panel_height - 2.0 * padding;

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

        // Create icon text format
        let icon_format =
            match self
                .renderer
                .create_text_format(&style.font_family, icon_size, false, false)
            {
                Ok(f) => f,
                Err(e) => {
                    log!("Failed to create task panel icon format: {:?}", e);
                    return;
                }
            };

        // Clear previous item states
        task_panel.item_states.clear();

        // Draw groups and tasks
        let mut y = panel_top + padding;
        let center_x = panel_left + panel_width_scaled / 2.0;

        // We need to clone the data we need since we can't borrow task_panel mutably while iterating
        let groups: Vec<_> = task_panel.config.groups.iter().cloned().collect();
        let expanded: Vec<_> = task_panel.expanded_groups.clone();
        let hovered = task_panel.hovered_item;
        let selected = task_panel.selected_item;
        let is_focused = task_panel.focused;

        // Circle background colors
        let circle_bg_normal = Color::from_f32(1.0, 1.0, 1.0, 0.1);
        let circle_bg_hover = Color::from_f32(1.0, 1.0, 1.0, 0.25);
        let circle_radius = icon_size / 2.0 + 4.0 * scale; // Slightly larger than icon
        let circle_diameter = circle_radius * 2.0;

        for (group_idx, group) in groups.iter().enumerate() {
            let is_expanded = expanded.get(group_idx).copied().unwrap_or(false);

            // Calculate circle center position
            let icon_center_x = center_x;
            let icon_center_y = y + circle_radius;

            // Check if this group header is hovered or selected
            let item_index = task_panel.item_states.len();
            let is_hovered = hovered == Some(item_index);
            let is_selected = is_focused && selected == Some(item_index);

            // Draw circular background
            let circle_color = if is_hovered || is_selected {
                circle_bg_hover
            } else {
                circle_bg_normal
            };
            let _ = self.renderer.fill_circle(
                icon_center_x,
                icon_center_y,
                circle_radius,
                circle_color,
            );

            let group_color = if is_hovered || is_selected {
                style.icon_color_hover
            } else {
                style.group_icon_color
            };

            // Draw group icon centered in circle
            let icon_rect = D2D_RECT_F {
                left: icon_center_x - circle_radius,
                top: icon_center_y - circle_radius,
                right: icon_center_x + circle_radius,
                bottom: icon_center_y + circle_radius,
            };
            let _ =
                self.renderer
                    .draw_text_centered(&group.icon, &icon_format, icon_rect, group_color);

            // Store item state for hit-testing (use circle bounds)
            task_panel.item_states.push(TaskItemState {
                group_index: group_idx,
                task_index: None,
                bounds: Rect::new(
                    icon_center_x - circle_radius,
                    icon_center_y - circle_radius,
                    circle_diameter,
                    circle_diameter,
                ),
                is_group_header: true,
            });

            y += circle_diameter + task_spacing;

            // Draw tasks if expanded
            if is_expanded {
                for (task_idx, task) in group.tasks.iter().enumerate() {
                    // Calculate circle center position
                    let task_center_x = center_x;
                    let task_center_y = y + circle_radius;

                    // Check if this task is hovered or selected
                    let item_index = task_panel.item_states.len();
                    let is_hovered = hovered == Some(item_index);
                    let is_selected = is_focused && selected == Some(item_index);

                    // Draw circular background
                    let circle_color = if is_hovered || is_selected {
                        circle_bg_hover
                    } else {
                        circle_bg_normal
                    };
                    let _ = self.renderer.fill_circle(
                        task_center_x,
                        task_center_y,
                        circle_radius,
                        circle_color,
                    );

                    let task_color = if is_hovered || is_selected {
                        style.icon_color_hover
                    } else {
                        style.icon_color
                    };

                    // Draw task icon centered in circle
                    let task_rect = D2D_RECT_F {
                        left: task_center_x - circle_radius,
                        top: task_center_y - circle_radius,
                        right: task_center_x + circle_radius,
                        bottom: task_center_y + circle_radius,
                    };
                    let _ = self.renderer.draw_text_centered(
                        &task.icon,
                        &icon_format,
                        task_rect,
                        task_color,
                    );

                    // Store item state for hit-testing (use circle bounds)
                    task_panel.item_states.push(TaskItemState {
                        group_index: group_idx,
                        task_index: Some(task_idx),
                        bounds: Rect::new(
                            task_center_x - circle_radius,
                            task_center_y - circle_radius,
                            circle_diameter,
                            circle_diameter,
                        ),
                        is_group_header: false,
                    });

                    y += circle_diameter + task_spacing;
                }
            }

            y += group_spacing - task_spacing; // Extra spacing between groups
        }

        // Draw tooltip if hovering over an item OR if panel is focused with keyboard selection
        // Collect tooltip data first to avoid borrow conflicts
        let tooltip_idx = if let Some(hovered_idx) = task_panel.hovered_item {
            // Mouse hover takes priority
            Some(hovered_idx)
        } else if task_panel.focused {
            // When focused, show tooltip for keyboard-selected item
            task_panel.selected_item
        } else {
            None
        };

        let tooltip_data: Option<(String, f32, f32)> = if let Some(idx) = tooltip_idx {
            if let Some((group, task)) = task_panel.get_task_at_index(idx) {
                let tooltip_text = if let Some(t) = task {
                    t.name.clone()
                } else {
                    group.name.clone()
                };

                if let Some(item_state) = task_panel.item_states.get(idx) {
                    Some((
                        tooltip_text,
                        item_state.bounds.x + item_state.bounds.width + 4.0 * scale,
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

        // Now draw tooltip without holding task_panel borrow
        if let Some((text, x, y)) = tooltip_data {
            self.draw_tooltip(&text, x, y, scale);
        }
    }

    /// Draw a tooltip near the given position
    fn draw_tooltip(&mut self, text: &str, x: f32, y: f32, scale: f32) {
        use windows::Win32::Graphics::Direct2D::Common::D2D_RECT_F;

        let font_size = 12.0 * scale;
        let padding = 8.0 * scale; // Increased padding for better appearance
        let bg_color = Color::from_f32(0.1, 0.1, 0.1, 0.95);
        let text_color = Color::WHITE;

        let text_format = match self.renderer.create_text_format(
            &self.task_panel_style.font_family,
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

    /// Toggle window visibility (called from hotkey)
    pub fn toggle_visibility(&mut self) {
        log!("toggle_visibility() called, is_visible={}", self.is_visible);

        // Toggle based on our tracked state, not IsWindowVisible
        // (IsWindowVisible can lag behind due to animation/async hide)
        if self.is_visible {
            // Hide the window
            log!("  Hiding window...");
            self.is_visible = false;
            self.textbox.clear();
            win32::hide_window(self.hwnd);
            self.stop_cursor_timer();
            self.stop_animation_timer();
            self.stop_clock_timer();
            self.animator.clear();
            log!("  Window hidden");
        } else {
            // Show the window
            log!("  Showing window...");
            self.is_visible = true;

            // Actually show the window
            win32::show_window(self.hwnd);

            // Reposition window with correct DPI scaling before showing content
            win32::reposition_window(self.hwnd, &self.config);

            // Force renderer to recreate buffers at new size
            let _ = self.renderer.handle_resize();

            // Clear cached bitmap - it was created on the old render target
            // and will cause D2D errors if used on the new one
            self.background_bitmap = None;
            self.background_bitmap_path = None;

            self.textbox.set_state(WidgetState::Focused);
            self.textbox.show_cursor();
            self.start_cursor_timer();

            // Initialize listview with all items
            self.listview.set_items(self.all_items.clone());

            // Mark renderer as dirty to force a full render on first frame
            self.renderer.mark_dirty();

            // Start fade-in animation
            // Note: Clock timer is started after animation completes to avoid flicker
            self.animator.start_fade_in();
            self.start_animation_timer();
            log!("  Started fade-in animation");

            // Force an immediate paint to render content at animation start opacity
            // This ensures we have rendered content before the animation timer starts
            // updating just the opacity
            self.paint();
            log!("  Window shown, initial paint complete");
        }
        log!("toggle_visibility() completed");
    }
}
