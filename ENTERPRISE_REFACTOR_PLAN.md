# Wolfy Enterprise Refactoring Plan
## Design Patterns: GoF, Clean Architecture, and SOLID Principles

**Version:** 1.0  
**Date:** 2026-02-01  
**Current Codebase:** ~60 Rust files, ~6,700 total lines

---

## Executive Summary

This document outlines a comprehensive refactoring strategy for Wolfy to align with enterprise-grade design patterns:
- **Gang of Four (GoF)** design patterns for extensibility and maintainability
- **Clean Architecture** for proper layer separation and dependency management
- **SOLID principles** for robust object-oriented design

### Current Architecture Assessment

**Strengths:**
- Clear module separation (theme, widget, platform, layout)
- Trait-based widget system provides extensibility
- Platform abstraction exists (win32 module)
- No Windows dependencies in core logic (lib.rs approach)

**Critical Issues:**
- **Monolithic App struct** (3,808 lines in app.rs) - violates Single Responsibility Principle
- **Tight coupling** between UI, business logic, and platform code
- **Missing abstraction layers** - no clear domain/use case boundaries
- **Direct Win32 dependencies** throughout application code
- **God object antipattern** - App manages everything (rendering, state, events, animations, tasks, etc.)
- **Limited testability** - hard to test without Windows environment
- **No dependency injection** - concrete implementations hardcoded

---

## 1. Clean Architecture Layer Model

We'll restructure Wolfy into concentric layers with strict dependency rules:

```
┌─────────────────────────────────────────────────────────┐
│                    Infrastructure                        │
│  (Frameworks, Win32 API, Direct2D, File System)         │
│  src/infrastructure/                                     │
└──────────────────────┬──────────────────────────────────┘
                       │ implements ↑
┌──────────────────────┴──────────────────────────────────┐
│               Interface Adapters                         │
│  (Controllers, Presenters, Gateways, Mappers)           │
│  src/adapters/                                           │
└──────────────────────┬──────────────────────────────────┘
                       │ implements ↑
┌──────────────────────┴──────────────────────────────────┐
│                Application/Use Cases                     │
│  (Business rules, workflows, orchestration)             │
│  src/application/                                        │
└──────────────────────┬──────────────────────────────────┘
                       │ uses ↑
┌──────────────────────┴──────────────────────────────────┐
│                   Domain/Entities                        │
│  (Core business logic, enterprise rules)                │
│  src/domain/                                             │
└─────────────────────────────────────────────────────────┘
```

### 1.1 Domain Layer (Core)

**Path:** `src/domain/`

Pure business logic with zero dependencies on external frameworks.

**Modules:**

```rust
// src/domain/entities/
pub mod app_item;        // Application entry entity
pub mod theme;           // Theme configuration entity
pub mod window_state;    // Window state entity
pub mod search_query;    // Search query value object
pub mod task;            // Task entity

// src/domain/value_objects/
pub mod color;           // Color value object (move from theme/types.rs)
pub mod rect;            // Rectangle value object
pub mod dimensions;      // Size/constraints value objects
pub mod hotkey;          // Hotkey configuration

// src/domain/repositories/ (interfaces only, no implementations)
pub trait AppRepository {
    fn discover_all(&self) -> Result<Vec<AppItem>, DomainError>;
    fn search(&self, query: &SearchQuery) -> Vec<AppItem>;
}

pub trait ThemeRepository {
    fn load(&self, name: &str) -> Result<Theme, DomainError>;
    fn list_available(&self) -> Vec<String>;
    fn watch_changes(&mut self) -> Result<(), DomainError>;
}

pub trait IconRepository {
    fn load_icon(&self, path: &Path) -> Result<Icon, DomainError>;
}

// src/domain/services/ (domain services for complex operations)
pub struct SearchService {
    // Fuzzy matching algorithm
    pub fn search(&self, items: &[AppItem], query: &SearchQuery) -> Vec<ScoredResult>;
}

pub struct ThemeResolverService {
    // CSS-like theme resolution logic
    pub fn resolve_property(&self, theme: &Theme, selector: &str, property: &str) -> Option<Value>;
}
```

**Benefits:**
- Zero external dependencies
- 100% testable without mocks
- Clear business rules
- Framework-agnostic

---

### 1.2 Application Layer (Use Cases)

**Path:** `src/application/`

Orchestrates domain entities and defines application-specific workflows.

**Use Case Examples:**

```rust
// src/application/use_cases/

// Use Case: Launch Application
pub struct LaunchApplicationUseCase<R: RuntimePort> {
    app_repository: Arc<dyn AppRepository>,
    runtime: R,
}

impl<R: RuntimePort> LaunchApplicationUseCase<R> {
    pub fn execute(&self, app_id: &str) -> Result<(), ApplicationError> {
        // 1. Find app
        let app = self.app_repository.find_by_id(app_id)?;
        
        // 2. Update history
        // 3. Launch via runtime
        self.runtime.execute(&app.path)?;
        
        // 4. Return result
        Ok(())
    }
}

// Use Case: Search Applications
pub struct SearchApplicationsUseCase {
    app_repository: Arc<dyn AppRepository>,
    search_service: SearchService,
    history: Arc<dyn HistoryRepository>,
}

impl SearchApplicationsUseCase {
    pub fn execute(&self, query: SearchQuery) -> Vec<AppItemDTO> {
        let all_apps = self.app_repository.discover_all()?;
        let history_boost = self.history.get_frequency_map();
        
        let mut results = self.search_service.search(&all_apps, &query);
        
        // Apply history boost
        for result in &mut results {
            if let Some(boost) = history_boost.get(&result.item.id) {
                result.score += boost;
            }
        }
        
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.into_iter().map(AppItemDTO::from).collect()
    }
}

// Use Case: Switch Theme
pub struct SwitchThemeUseCase {
    theme_repository: Arc<dyn ThemeRepository>,
    config_repository: Arc<dyn ConfigRepository>,
    event_bus: Arc<dyn EventBus>,
}

impl SwitchThemeUseCase {
    pub fn execute(&self, theme_name: &str) -> Result<(), ApplicationError> {
        let theme = self.theme_repository.load(theme_name)?;
        self.config_repository.set_current_theme(theme_name)?;
        self.event_bus.publish(Event::ThemeChanged(theme));
        Ok(())
    }
}

// Use Case: Animate Window
pub struct AnimateWindowUseCase {
    animator: Arc<dyn AnimationPort>,
}

impl AnimateWindowUseCase {
    pub fn execute(&self, from: WindowState, to: WindowState, duration_ms: u32) {
        self.animator.animate(from, to, duration_ms, Easing::EaseOut);
    }
}
```

**Application Services:**

```rust
// src/application/services/

pub struct ApplicationCommandHandler {
    // Coordinates multiple use cases
    launch_app_uc: LaunchApplicationUseCase,
    search_uc: SearchApplicationsUseCase,
    // ... other use cases
}

pub struct ThemeManager {
    // Manages theme lifecycle
    load_theme_uc: LoadThemeUseCase,
    switch_theme_uc: SwitchThemeUseCase,
    watch_theme_uc: WatchThemeUseCase,
}

pub struct WindowManager {
    // Manages window lifecycle
    show_window_uc: ShowWindowUseCase,
    hide_window_uc: HideWindowUseCase,
    animate_window_uc: AnimateWindowUseCase,
}
```

**Ports (Interfaces):**

```rust
// src/application/ports/

// Output ports (implemented by infrastructure)
pub trait RuntimePort {
    fn execute(&self, path: &str) -> Result<(), RuntimeError>;
    fn spawn_terminal(&self, command: &str) -> Result<ProcessHandle, RuntimeError>;
}

pub trait RenderPort {
    fn render(&mut self, scene: &RenderScene) -> Result<(), RenderError>;
    fn create_texture(&mut self, image_data: &[u8]) -> Result<TextureId, RenderError>;
}

pub trait AnimationPort {
    fn animate(&self, from: WindowState, to: WindowState, duration: u32, easing: Easing);
}

pub trait FileSystemPort {
    fn read_file(&self, path: &Path) -> Result<Vec<u8>, IoError>;
    fn watch_file(&mut self, path: &Path) -> Result<WatchHandle, IoError>;
}

// Input ports (implemented by adapters, called by infrastructure)
pub trait ApplicationCommandPort {
    fn handle_search(&mut self, query: &str);
    fn handle_select(&mut self, index: usize);
    fn handle_launch(&mut self, app_id: &str);
    fn handle_close(&mut self);
}

pub trait WindowEventPort {
    fn on_show(&mut self);
    fn on_hide(&mut self);
    fn on_resize(&mut self, width: u32, height: u32);
}
```

---

### 1.3 Interface Adapters Layer

**Path:** `src/adapters/`

Converts data between use case format and external format (UI, Win32, files, etc.).

**Controllers:**

```rust
// src/adapters/controllers/

// Receives input events from Win32, translates to use case calls
pub struct WindowController {
    command_handler: Arc<ApplicationCommandHandler>,
    window_manager: Arc<WindowManager>,
}

impl WindowController {
    pub fn handle_win32_message(&mut self, msg: u32, wparam: WPARAM, lparam: LPARAM) {
        match msg {
            WM_KEYDOWN => {
                let key_code = self.translate_key(wparam);
                match key_code {
                    KeyCode::Enter => {
                        self.command_handler.handle_launch_selected();
                    }
                    KeyCode::Escape => {
                        self.command_handler.handle_close();
                    }
                    _ => {}
                }
            }
            WM_HOTKEY => {
                let hotkey_id = wparam.0 as usize;
                self.handle_hotkey(hotkey_id);
            }
            _ => {}
        }
    }
}

pub struct SearchController {
    search_uc: Arc<SearchApplicationsUseCase>,
    presenter: Arc<Mutex<SearchPresenter>>,
}

impl SearchController {
    pub fn search(&mut self, query: &str) {
        let results = self.search_uc.execute(SearchQuery::new(query));
        self.presenter.lock().unwrap().present_results(results);
    }
}
```

**Presenters:**

```rust
// src/adapters/presenters/

// Formats use case output for UI display
pub struct SearchPresenter {
    view: Arc<Mutex<dyn SearchView>>,
}

impl SearchPresenter {
    pub fn present_results(&mut self, results: Vec<AppItemDTO>) {
        let view_models: Vec<ListItemViewModel> = results
            .into_iter()
            .map(|dto| ListItemViewModel {
                id: dto.id,
                title: dto.name,
                subtitle: dto.path.to_string_lossy().to_string(),
                icon: dto.icon_path,
            })
            .collect();
        
        self.view.lock().unwrap().display_items(view_models);
    }
}

pub struct ThemePresenter {
    view: Arc<Mutex<dyn ThemeView>>,
}

impl ThemePresenter {
    pub fn present_theme(&mut self, theme: ThemeDTO) {
        let colors = ViewColorScheme {
            background: self.convert_color(theme.background_color),
            text: self.convert_color(theme.text_color),
            // ...
        };
        self.view.lock().unwrap().apply_colors(colors);
    }
}
```

**Gateways (Repository Implementations):**

```rust
// src/adapters/gateways/

pub struct Win32AppGateway {
    icon_loader: Arc<Win32IconLoader>,
}

impl AppRepository for Win32AppGateway {
    fn discover_all(&self) -> Result<Vec<AppItem>, DomainError> {
        // Scan Start Menu, desktop, etc.
        // Parse .lnk files
        // Load icons
        Ok(apps)
    }
}

pub struct RasiThemeGateway {
    parser: RasiParser,
    file_system: Arc<dyn FileSystemPort>,
}

impl ThemeRepository for RasiThemeGateway {
    fn load(&self, name: &str) -> Result<Theme, DomainError> {
        let path = self.resolve_theme_path(name)?;
        let content = self.file_system.read_file(&path)?;
        let ast = self.parser.parse(&content)?;
        Ok(Theme::from_ast(ast))
    }
}

pub struct FileHistoryGateway {
    path: PathBuf,
}

impl HistoryRepository for FileHistoryGateway {
    fn record_launch(&mut self, app_id: &str) -> Result<(), DomainError> {
        // Update history file
        Ok(())
    }
    
    fn get_frequency_map(&self) -> HashMap<String, f32> {
        // Load and parse history file
        HashMap::new()
    }
}
```

**View Interfaces:**

```rust
// src/adapters/views/

pub trait SearchView {
    fn display_items(&mut self, items: Vec<ListItemViewModel>);
    fn highlight_item(&mut self, index: usize);
    fn clear(&mut self);
}

pub trait ThemeView {
    fn apply_colors(&mut self, colors: ViewColorScheme);
    fn set_opacity(&mut self, opacity: f32);
}
```

---

### 1.4 Infrastructure Layer

**Path:** `src/infrastructure/`

Concrete implementations of all interfaces, external framework integrations.

**Structure:**

```rust
// src/infrastructure/

pub mod win32/          // All Windows-specific code
    mod window_impl;    // Window creation, message loop
    mod d2d_renderer;   // Direct2D rendering implementation
    mod hotkey_impl;    // Hotkey registration
    mod icon_loader;    // Icon loading via Win32 API
    mod app_discovery;  // Start Menu scanning
    mod runtime;        // Process spawning

pub mod filesystem/
    mod file_watcher;   // File watching implementation
    mod config_loader;  // Config file loading

pub mod animation/
    mod animator;       // Animation system implementation

pub mod parser/
    mod rasi_parser;    // LALRPOP-based theme parser
    mod lexer;          // Lexer for themes

// Wire everything together
pub mod composition_root;  // Dependency injection container
```

**Dependency Injection Container:**

```rust
// src/infrastructure/composition_root.rs

pub struct CompositionRoot {
    // Repositories
    app_repository: Arc<dyn AppRepository>,
    theme_repository: Arc<dyn ThemeRepository>,
    history_repository: Arc<dyn HistoryRepository>,
    
    // Ports
    render_port: Arc<dyn RenderPort>,
    runtime_port: Arc<dyn RuntimePort>,
    animation_port: Arc<dyn AnimationPort>,
    
    // Services
    search_service: Arc<SearchService>,
    theme_resolver: Arc<ThemeResolverService>,
    
    // Use Cases
    launch_app_uc: Arc<LaunchApplicationUseCase>,
    search_apps_uc: Arc<SearchApplicationsUseCase>,
    switch_theme_uc: Arc<SwitchThemeUseCase>,
    
    // Controllers
    window_controller: Arc<Mutex<WindowController>>,
    search_controller: Arc<Mutex<SearchController>>,
}

impl CompositionRoot {
    pub fn new() -> Result<Self, InitError> {
        // Create infrastructure implementations
        let icon_loader = Arc::new(Win32IconLoader::new()?);
        let file_system = Arc::new(Win32FileSystem::new());
        
        // Create gateways
        let app_repository: Arc<dyn AppRepository> = 
            Arc::new(Win32AppGateway::new(icon_loader.clone()));
        let theme_repository: Arc<dyn ThemeRepository> = 
            Arc::new(RasiThemeGateway::new(file_system.clone()));
        let history_repository: Arc<dyn HistoryRepository> = 
            Arc::new(FileHistoryGateway::new());
        
        // Create ports
        let render_port: Arc<dyn RenderPort> = 
            Arc::new(D2DRenderer::new()?);
        let runtime_port: Arc<dyn RuntimePort> = 
            Arc::new(Win32Runtime::new());
        let animation_port: Arc<dyn AnimationPort> = 
            Arc::new(WindowAnimator::new());
        
        // Create domain services
        let search_service = Arc::new(SearchService::new());
        let theme_resolver = Arc::new(ThemeResolverService::new());
        
        // Create use cases
        let launch_app_uc = Arc::new(LaunchApplicationUseCase::new(
            app_repository.clone(),
            runtime_port.clone(),
        ));
        let search_apps_uc = Arc::new(SearchApplicationsUseCase::new(
            app_repository.clone(),
            search_service.clone(),
            history_repository.clone(),
        ));
        let switch_theme_uc = Arc::new(SwitchThemeUseCase::new(
            theme_repository.clone(),
        ));
        
        // Create application services
        let command_handler = Arc::new(ApplicationCommandHandler::new(
            launch_app_uc.clone(),
            search_apps_uc.clone(),
        ));
        let window_manager = Arc::new(WindowManager::new(
            animation_port.clone(),
        ));
        
        // Create controllers
        let window_controller = Arc::new(Mutex::new(WindowController::new(
            command_handler.clone(),
            window_manager.clone(),
        )));
        let search_controller = Arc::new(Mutex::new(SearchController::new(
            search_apps_uc.clone(),
        )));
        
        Ok(Self {
            app_repository,
            theme_repository,
            history_repository,
            render_port,
            runtime_port,
            animation_port,
            search_service,
            theme_resolver,
            launch_app_uc,
            search_apps_uc,
            switch_theme_uc,
            window_controller,
            search_controller,
        })
    }
    
    pub fn window_controller(&self) -> Arc<Mutex<WindowController>> {
        self.window_controller.clone()
    }
}
```

**Main Entry Point:**

```rust
// src/main.rs (simplified)

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_dpi_awareness();
    
    // Create dependency injection container
    let container = CompositionRoot::new()?;
    
    // Create window with controller callback
    let window_controller = container.window_controller();
    let hwnd = create_window_with_callback(move |hwnd, msg, wparam, lparam| {
        window_controller.lock().unwrap().handle_win32_message(msg, wparam, lparam)
    })?;
    
    // Register hotkeys
    register_hotkeys()?;
    
    // Run message loop
    run_message_loop()?;
    
    Ok(())
}
```

---

## 2. Gang of Four (GoF) Design Patterns

### 2.1 Creational Patterns

#### **Abstract Factory Pattern** - Widget Creation

**Problem:** Currently widget creation is scattered. We need to create families of related widgets (standard, themed, grid).

```rust
// src/domain/factories/

pub trait WidgetFactory {
    fn create_window(&self) -> Box<dyn Widget>;
    fn create_listview(&self) -> Box<dyn Widget>;
    fn create_textbox(&self) -> Box<dyn Widget>;
    fn create_panel(&self) -> Box<dyn Widget>;
}

pub struct LauncherWidgetFactory {
    theme: Arc<Theme>,
}

impl WidgetFactory for LauncherWidgetFactory {
    fn create_window(&self) -> Box<dyn Widget> {
        Box::new(Window::new(self.theme.clone()))
    }
    
    fn create_listview(&self) -> Box<dyn Widget> {
        Box::new(ListView::new(self.theme.clone()))
    }
    // ...
}

pub struct GridWidgetFactory {
    theme: Arc<Theme>,
}

impl WidgetFactory for GridWidgetFactory {
    fn create_window(&self) -> Box<dyn Widget> {
        Box::new(GridWindow::new(self.theme.clone()))
    }
    
    fn create_listview(&self) -> Box<dyn Widget> {
        Box::new(GridView::new(self.theme.clone()))
    }
}
```

#### **Builder Pattern** - Complex Object Construction

**Problem:** Window configuration, theme configuration, widget trees have many optional parameters.

```rust
// src/domain/builders/

pub struct WindowConfigBuilder {
    width: Option<u32>,
    height: Option<u32>,
    opacity: f32,
    mode: Mode,
    theme: Option<String>,
    // ... many more options
}

impl WindowConfigBuilder {
    pub fn new() -> Self { /* ... */ }
    
    pub fn width(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }
    
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
    
    pub fn theme(mut self, theme: impl Into<String>) -> Self {
        self.theme = Some(theme.into());
        self
    }
    
    pub fn build(self) -> Result<WindowConfig, ValidationError> {
        Ok(WindowConfig {
            width: self.width.unwrap_or(928),
            height: self.height.unwrap_or(600),
            opacity: self.opacity,
            // ...
        })
    }
}

// Usage:
let config = WindowConfigBuilder::new()
    .width(1200)
    .opacity(0.95)
    .theme("launcher")
    .build()?;
```

#### **Factory Method Pattern** - Extensible Widget Creation

**Problem:** Widget type determination and instantiation is hardcoded.

```rust
// src/domain/factories/

pub trait WidgetCreator {
    fn create_widget(&self, widget_type: &str, config: WidgetConfig) -> Result<Box<dyn Widget>, FactoryError>;
}

pub struct DefaultWidgetCreator {
    theme: Arc<Theme>,
}

impl WidgetCreator for DefaultWidgetCreator {
    fn create_widget(&self, widget_type: &str, config: WidgetConfig) -> Result<Box<dyn Widget>, FactoryError> {
        match widget_type {
            "listview" => Ok(Box::new(ListView::new(config, self.theme.clone()))),
            "textbox" => Ok(Box::new(Textbox::new(config, self.theme.clone()))),
            "panel" => Ok(Box::new(Panel::new(config, self.theme.clone()))),
            _ => Err(FactoryError::UnknownWidgetType(widget_type.to_string()))
        }
    }
}

// Allows plugins to register custom widget types
pub struct ExtensibleWidgetCreator {
    default: DefaultWidgetCreator,
    extensions: HashMap<String, Box<dyn Fn(WidgetConfig) -> Box<dyn Widget>>>,
}
```

#### **Singleton Pattern** - Application State (Use Sparingly!)

**Problem:** Some resources should only have one instance (compositor, event bus).

```rust
// src/infrastructure/event_bus.rs

use std::sync::{Arc, Mutex, Once};

static mut EVENT_BUS: Option<Arc<Mutex<EventBus>>> = None;
static INIT: Once = Once::new();

pub struct EventBus {
    subscribers: Vec<Box<dyn EventSubscriber>>,
}

impl EventBus {
    pub fn instance() -> Arc<Mutex<EventBus>> {
        unsafe {
            INIT.call_once(|| {
                EVENT_BUS = Some(Arc::new(Mutex::new(EventBus {
                    subscribers: Vec::new(),
                })));
            });
            EVENT_BUS.clone().unwrap()
        }
    }
}

// Better alternative: Pass via dependency injection
// Only use Singleton for truly global resources
```

#### **Prototype Pattern** - Clone Theme Configurations

**Problem:** Need to create variations of themes efficiently.

```rust
// src/domain/entities/

pub trait Cloneable {
    fn clone_box(&self) -> Box<dyn Theme>;
}

impl Cloneable for RasiTheme {
    fn clone_box(&self) -> Box<dyn Theme> {
        Box::new(self.clone())
    }
}

pub struct ThemePrototypeRegistry {
    prototypes: HashMap<String, Box<dyn Cloneable>>,
}

impl ThemePrototypeRegistry {
    pub fn register(&mut self, name: &str, prototype: Box<dyn Cloneable>) {
        self.prototypes.insert(name.to_string(), prototype);
    }
    
    pub fn create(&self, name: &str) -> Option<Box<dyn Theme>> {
        self.prototypes.get(name).map(|p| p.clone_box())
    }
}
```

---

### 2.2 Structural Patterns

#### **Adapter Pattern** - Platform Abstraction

**Problem:** Need to use Win32 API but keep domain layer platform-agnostic.

```rust
// src/adapters/platform/

// Domain interface
pub trait WindowSystemAdapter {
    fn create_window(&self, config: WindowConfig) -> Result<WindowHandle, PlatformError>;
    fn show_window(&self, handle: WindowHandle);
    fn hide_window(&self, handle: WindowHandle);
}

// Win32 implementation
pub struct Win32WindowAdapter {
    // Win32-specific state
}

impl WindowSystemAdapter for Win32WindowAdapter {
    fn create_window(&self, config: WindowConfig) -> Result<WindowHandle, PlatformError> {
        // Translate domain WindowConfig to Win32 WNDCLASSEX
        let hwnd = unsafe { CreateWindowExW(/* ... */) };
        Ok(WindowHandle::from_raw(hwnd.0))
    }
    
    fn show_window(&self, handle: WindowHandle) {
        unsafe { ShowWindow(HWND(handle.raw()), SW_SHOW) };
    }
}

// Future: Linux/macOS adapters can implement the same interface
pub struct WaylandWindowAdapter { /* ... */ }
pub struct CocoaWindowAdapter { /* ... */ }
```

#### **Bridge Pattern** - Separate Abstraction from Implementation

**Problem:** Rendering logic should be independent of rendering backend (Direct2D, OpenGL, Vulkan).

```rust
// src/domain/rendering/

// Abstraction
pub trait RenderBackend {
    fn begin_frame(&mut self);
    fn end_frame(&mut self);
    fn draw_rect(&mut self, rect: Rect, color: Color);
    fn draw_text(&mut self, text: &str, pos: Point, style: TextStyle);
    fn draw_image(&mut self, image_id: ImageId, rect: Rect);
}

// Implementation 1: Direct2D
pub struct D2DRenderBackend {
    context: ID2D1DeviceContext,
    // ... Direct2D resources
}

impl RenderBackend for D2DRenderBackend {
    fn draw_rect(&mut self, rect: Rect, color: Color) {
        let d2d_rect = D2D1_RECT_F { /* convert */ };
        let brush = self.create_solid_brush(color);
        unsafe { self.context.FillRectangle(&d2d_rect, &brush); }
    }
}

// Implementation 2: Future Skia backend
pub struct SkiaRenderBackend {
    canvas: skia_safe::Canvas,
}

impl RenderBackend for SkiaRenderBackend {
    fn draw_rect(&mut self, rect: Rect, color: Color) {
        let skia_rect = skia_safe::Rect::from_xywh(/* ... */);
        let paint = skia_safe::Paint::new(color, None);
        self.canvas.draw_rect(skia_rect, &paint);
    }
}

// Refined abstraction
pub struct SceneRenderer {
    backend: Box<dyn RenderBackend>,
}

impl SceneRenderer {
    pub fn render(&mut self, scene: &Scene) {
        self.backend.begin_frame();
        for drawable in &scene.drawables {
            self.render_drawable(drawable);
        }
        self.backend.end_frame();
    }
}
```

#### **Composite Pattern** - Widget Tree Hierarchy

**Problem:** Need to treat individual widgets and widget containers uniformly.

```rust
// src/domain/widgets/

pub trait Widget {
    fn render(&self, renderer: &mut dyn RenderBackend);
    fn handle_event(&mut self, event: &Event) -> EventResult;
    fn measure(&self, constraints: Constraints) -> Size;
    fn arrange(&mut self, bounds: Rect);
    
    // Composite methods
    fn add_child(&mut self, child: Box<dyn Widget>) -> Result<(), WidgetError> {
        Err(WidgetError::NotAContainer)
    }
    
    fn remove_child(&mut self, index: usize) -> Result<(), WidgetError> {
        Err(WidgetError::NotAContainer)
    }
    
    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }
}

// Leaf widget
pub struct Textbox {
    // No children
}

impl Widget for Textbox {
    fn render(&self, renderer: &mut dyn RenderBackend) {
        renderer.draw_rect(self.bounds, self.background);
        renderer.draw_text(&self.text, self.position, self.style);
    }
}

// Composite widget
pub struct Container {
    children: Vec<Box<dyn Widget>>,
    layout: LayoutStrategy,
}

impl Widget for Container {
    fn render(&self, renderer: &mut dyn RenderBackend) {
        renderer.draw_rect(self.bounds, self.background);
        for child in &self.children {
            child.render(renderer);
        }
    }
    
    fn add_child(&mut self, child: Box<dyn Widget>) -> Result<(), WidgetError> {
        self.children.push(child);
        Ok(())
    }
    
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
}
```

#### **Decorator Pattern** - Widget Enhancement

**Problem:** Need to add behavior to widgets (scrolling, borders, shadows) without modifying widget classes.

```rust
// src/domain/widgets/decorators/

pub struct ScrollableDecorator {
    inner: Box<dyn Widget>,
    scroll_offset: f32,
    max_scroll: f32,
}

impl Widget for ScrollableDecorator {
    fn render(&self, renderer: &mut dyn RenderBackend) {
        renderer.push_clip(self.bounds);
        renderer.push_transform(Translation::new(0.0, -self.scroll_offset));
        self.inner.render(renderer);
        renderer.pop_transform();
        renderer.pop_clip();
        
        // Draw scrollbar
        self.draw_scrollbar(renderer);
    }
    
    fn handle_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::MouseWheel(delta) => {
                self.scroll_offset = (self.scroll_offset - delta).clamp(0.0, self.max_scroll);
                EventResult::Handled
            }
            _ => self.inner.handle_event(event)
        }
    }
}

pub struct BorderDecorator {
    inner: Box<dyn Widget>,
    border_color: Color,
    border_width: f32,
    border_radius: f32,
}

impl Widget for BorderDecorator {
    fn render(&self, renderer: &mut dyn RenderBackend) {
        self.inner.render(renderer);
        renderer.draw_rounded_rect(self.bounds, self.border_radius, self.border_color, self.border_width);
    }
}

// Usage:
let textbox = Box::new(Textbox::new());
let with_border = Box::new(BorderDecorator::new(textbox, Color::WHITE, 1.0, 4.0));
let with_scroll = Box::new(ScrollableDecorator::new(with_border));
```

#### **Facade Pattern** - Simplified API for Complex Subsystems

**Problem:** Win32 API is complex; need a simpler interface.

```rust
// src/adapters/platform/

pub struct WindowFacade {
    window_system: Box<dyn WindowSystemAdapter>,
    renderer: Box<dyn RenderBackend>,
    input_system: Box<dyn InputSystemAdapter>,
}

impl WindowFacade {
    pub fn new() -> Result<Self, PlatformError> {
        Ok(Self {
            window_system: Box::new(Win32WindowAdapter::new()?),
            renderer: Box::new(D2DRenderBackend::new()?),
            input_system: Box::new(Win32InputAdapter::new()),
        })
    }
    
    // Simple unified interface
    pub fn create_window(&mut self, config: WindowConfig) -> Result<WindowHandle, PlatformError> {
        let handle = self.window_system.create_window(config)?;
        self.renderer.attach_to_window(handle)?;
        self.input_system.register_window(handle)?;
        Ok(handle)
    }
    
    pub fn show(&mut self, handle: WindowHandle) {
        self.window_system.show_window(handle);
    }
    
    pub fn render_frame(&mut self, scene: &Scene) {
        self.renderer.begin_frame();
        for drawable in &scene.drawables {
            // Render logic
        }
        self.renderer.end_frame();
    }
}
```

#### **Flyweight Pattern** - Share Common Data (Icons, Fonts)

**Problem:** Loading duplicate icons/fonts wastes memory.

```rust
// src/infrastructure/caching/

pub struct IconFlyweightFactory {
    cache: HashMap<PathBuf, Arc<LoadedIcon>>,
    loader: Box<dyn IconLoader>,
}

impl IconFlyweightFactory {
    pub fn get_icon(&mut self, path: &Path) -> Result<Arc<LoadedIcon>, LoadError> {
        if let Some(icon) = self.cache.get(path) {
            return Ok(icon.clone());
        }
        
        let icon = Arc::new(self.loader.load(path)?);
        self.cache.insert(path.to_path_buf(), icon.clone());
        Ok(icon)
    }
}

// Each widget holds Arc<LoadedIcon> instead of raw icon data
pub struct AppItem {
    name: String,
    path: PathBuf,
    icon: Arc<LoadedIcon>,  // Shared!
}
```

#### **Proxy Pattern** - Lazy Loading, Access Control

**Problem:** Don't load all icons upfront; load on demand.

```rust
// src/domain/proxies/

pub trait Icon {
    fn get_texture(&self) -> Result<TextureId, LoadError>;
}

pub struct RealIcon {
    texture: TextureId,
}

impl Icon for RealIcon {
    fn get_texture(&self) -> Result<TextureId, LoadError> {
        Ok(self.texture)
    }
}

pub struct IconProxy {
    path: PathBuf,
    real_icon: Option<RealIcon>,
    loader: Arc<dyn IconLoader>,
}

impl Icon for IconProxy {
    fn get_texture(&self) -> Result<TextureId, LoadError> {
        if self.real_icon.is_none() {
            // Lazy load
            let real = self.loader.load(&self.path)?;
            self.real_icon = Some(real);
        }
        self.real_icon.as_ref().unwrap().get_texture()
    }
}
```

---

### 2.3 Behavioral Patterns

#### **Chain of Responsibility Pattern** - Event Handling

**Problem:** Events should bubble through widget hierarchy until handled.

```rust
// src/domain/events/

pub enum EventResult {
    Handled,
    NotHandled,
    Propagate,
}

pub trait EventHandler {
    fn handle_event(&mut self, event: &Event) -> EventResult;
    fn set_next(&mut self, next: Box<dyn EventHandler>);
}

pub struct WidgetEventHandler {
    widget: Box<dyn Widget>,
    next: Option<Box<dyn EventHandler>>,
}

impl EventHandler for WidgetEventHandler {
    fn handle_event(&mut self, event: &Event) -> EventResult {
        let result = self.widget.handle_event(event);
        
        match result {
            EventResult::Handled => EventResult::Handled,
            EventResult::NotHandled | EventResult::Propagate => {
                if let Some(ref mut next) = self.next {
                    next.handle_event(event)
                } else {
                    EventResult::NotHandled
                }
            }
        }
    }
    
    fn set_next(&mut self, next: Box<dyn EventHandler>) {
        self.next = Some(next);
    }
}
```

#### **Command Pattern** - Undo/Redo, Action Queue

**Problem:** Need to decouple action invocation from execution; support undo.

```rust
// src/domain/commands/

pub trait Command {
    fn execute(&mut self) -> Result<(), CommandError>;
    fn undo(&mut self) -> Result<(), CommandError>;
}

pub struct LaunchAppCommand {
    app_id: String,
    runtime: Arc<dyn RuntimePort>,
}

impl Command for LaunchAppCommand {
    fn execute(&mut self) -> Result<(), CommandError> {
        self.runtime.execute(&self.app_id)?;
        Ok(())
    }
    
    fn undo(&mut self) -> Result<(), CommandError> {
        // Can't undo launching an app, but log it
        Ok(())
    }
}

pub struct ChangeThemeCommand {
    old_theme: String,
    new_theme: String,
    theme_manager: Arc<ThemeManager>,
}

impl Command for ChangeThemeCommand {
    fn execute(&mut self) -> Result<(), CommandError> {
        self.theme_manager.switch_theme(&self.new_theme)?;
        Ok(())
    }
    
    fn undo(&mut self) -> Result<(), CommandError> {
        self.theme_manager.switch_theme(&self.old_theme)?;
        Ok(())
    }
}

pub struct CommandQueue {
    history: Vec<Box<dyn Command>>,
    current: usize,
}

impl CommandQueue {
    pub fn execute(&mut self, mut command: Box<dyn Command>) -> Result<(), CommandError> {
        command.execute()?;
        self.history.truncate(self.current);
        self.history.push(command);
        self.current += 1;
        Ok(())
    }
    
    pub fn undo(&mut self) -> Result<(), CommandError> {
        if self.current > 0 {
            self.current -= 1;
            self.history[self.current].undo()?;
        }
        Ok(())
    }
    
    pub fn redo(&mut self) -> Result<(), CommandError> {
        if self.current < self.history.len() {
            self.history[self.current].execute()?;
            self.current += 1;
        }
        Ok(())
    }
}
```

#### **Iterator Pattern** - Widget Traversal

**Problem:** Need to traverse widget tree in various orders (depth-first, breadth-first).

```rust
// src/domain/widgets/

pub struct DepthFirstWidgetIterator<'a> {
    stack: Vec<&'a dyn Widget>,
}

impl<'a> DepthFirstWidgetIterator<'a> {
    pub fn new(root: &'a dyn Widget) -> Self {
        Self { stack: vec![root] }
    }
}

impl<'a> Iterator for DepthFirstWidgetIterator<'a> {
    type Item = &'a dyn Widget;
    
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(widget) = self.stack.pop() {
            // Push children in reverse order for correct traversal
            for child in widget.children().iter().rev() {
                self.stack.push(child.as_ref());
            }
            Some(widget)
        } else {
            None
        }
    }
}

// Usage:
let root = create_widget_tree();
for widget in DepthFirstWidgetIterator::new(root.as_ref()) {
    println!("Widget: {:?}", widget);
}
```

#### **Mediator Pattern** - Decouple Widget Communication

**Problem:** Widgets shouldn't know about each other; mediator coordinates interactions.

```rust
// src/application/mediators/

pub trait Mediator {
    fn notify(&mut self, sender: &str, event: MediatorEvent);
}

pub struct LauncherMediator {
    textbox: Option<Weak<RefCell<Textbox>>>,
    listview: Option<Weak<RefCell<ListView>>>,
    search_uc: Arc<SearchApplicationsUseCase>,
}

impl Mediator for LauncherMediator {
    fn notify(&mut self, sender: &str, event: MediatorEvent) {
        match (sender, event) {
            ("textbox", MediatorEvent::TextChanged(text)) => {
                // Update search results
                let results = self.search_uc.execute(SearchQuery::new(&text));
                if let Some(listview) = self.listview.as_ref().and_then(|w| w.upgrade()) {
                    listview.borrow_mut().set_items(results);
                }
            }
            ("listview", MediatorEvent::ItemSelected(index)) => {
                // Handle selection, maybe update textbox
            }
            _ => {}
        }
    }
}

// Widgets hold reference to mediator
pub struct Textbox {
    text: String,
    mediator: Arc<Mutex<dyn Mediator>>,
}

impl Textbox {
    fn on_text_changed(&mut self) {
        self.mediator.lock().unwrap().notify("textbox", MediatorEvent::TextChanged(self.text.clone()));
    }
}
```

#### **Memento Pattern** - Save/Restore State

**Problem:** Need to save/restore application state (window position, search history, etc.).

```rust
// src/domain/memento/

pub struct AppMemento {
    query: String,
    selected_index: usize,
    scroll_position: f32,
    window_position: (i32, i32),
}

pub struct AppState {
    query: String,
    selected_index: usize,
    scroll_position: f32,
    window_position: (i32, i32),
}

impl AppState {
    pub fn save(&self) -> AppMemento {
        AppMemento {
            query: self.query.clone(),
            selected_index: self.selected_index,
            scroll_position: self.scroll_position,
            window_position: self.window_position,
        }
    }
    
    pub fn restore(&mut self, memento: AppMemento) {
        self.query = memento.query;
        self.selected_index = memento.selected_index;
        self.scroll_position = memento.scroll_position;
        self.window_position = memento.window_position;
    }
}

pub struct StateCaretaker {
    mementos: Vec<AppMemento>,
}

impl StateCaretaker {
    pub fn save(&mut self, state: &AppState) {
        self.mementos.push(state.save());
    }
    
    pub fn restore(&mut self, state: &mut AppState) {
        if let Some(memento) = self.mementos.pop() {
            state.restore(memento);
        }
    }
}
```

#### **Observer Pattern** - Event Bus, Theme Changes

**Problem:** Multiple components need to react to theme changes, window events, etc.

```rust
// src/domain/events/

pub trait Observer {
    fn update(&mut self, event: &DomainEvent);
}

pub trait Observable {
    fn attach(&mut self, observer: Box<dyn Observer>);
    fn detach(&mut self, observer_id: usize);
    fn notify(&mut self, event: DomainEvent);
}

pub enum DomainEvent {
    ThemeChanged(Theme),
    AppsUpdated(Vec<AppItem>),
    WindowResized(u32, u32),
}

pub struct EventBus {
    observers: Vec<Box<dyn Observer>>,
}

impl Observable for EventBus {
    fn attach(&mut self, observer: Box<dyn Observer>) {
        self.observers.push(observer);
    }
    
    fn notify(&mut self, event: DomainEvent) {
        for observer in &mut self.observers {
            observer.update(&event);
        }
    }
}

// Observer implementations
pub struct ThemeObserver {
    renderer: Arc<Mutex<dyn RenderPort>>,
}

impl Observer for ThemeObserver {
    fn update(&mut self, event: &DomainEvent) {
        if let DomainEvent::ThemeChanged(theme) = event {
            // Update renderer with new theme colors
            self.renderer.lock().unwrap().update_theme(theme);
        }
    }
}
```

#### **State Pattern** - Window State Machine

**Problem:** Window has different behaviors in different states (hidden, showing, shown, hiding).

```rust
// src/domain/states/

pub trait WindowState {
    fn show(&mut self, context: &mut WindowContext) -> Box<dyn WindowState>;
    fn hide(&mut self, context: &mut WindowContext) -> Box<dyn WindowState>;
    fn update(&mut self, context: &mut WindowContext, delta_ms: f32) -> Option<Box<dyn WindowState>>;
}

pub struct HiddenState;

impl WindowState for HiddenState {
    fn show(&mut self, context: &mut WindowContext) -> Box<dyn WindowState> {
        context.start_show_animation();
        Box::new(ShowingState::new())
    }
    
    fn hide(&mut self, _context: &mut WindowContext) -> Box<dyn WindowState> {
        // Already hidden
        Box::new(HiddenState)
    }
    
    fn update(&mut self, _context: &mut WindowContext, _delta_ms: f32) -> Option<Box<dyn WindowState>> {
        None
    }
}

pub struct ShowingState {
    animation_progress: f32,
}

impl WindowState for ShowingState {
    fn show(&mut self, _context: &mut WindowContext) -> Box<dyn WindowState> {
        // Already showing
        Box::new(ShowingState { animation_progress: self.animation_progress })
    }
    
    fn hide(&mut self, context: &mut WindowContext) -> Box<dyn WindowState> {
        context.start_hide_animation();
        Box::new(HidingState::new())
    }
    
    fn update(&mut self, context: &mut WindowContext, delta_ms: f32) -> Option<Box<dyn WindowState>> {
        self.animation_progress += delta_ms / context.animation_duration();
        
        if self.animation_progress >= 1.0 {
            context.complete_show_animation();
            Some(Box::new(ShownState))
        } else {
            context.update_opacity(self.animation_progress);
            None
        }
    }
}

pub struct WindowContext {
    state: Box<dyn WindowState>,
}

impl WindowContext {
    pub fn show(&mut self) {
        let new_state = self.state.show(self);
        self.state = new_state;
    }
    
    pub fn update(&mut self, delta_ms: f32) {
        if let Some(new_state) = self.state.update(self, delta_ms) {
            self.state = new_state;
        }
    }
}
```

#### **Strategy Pattern** - Layout Algorithms, Search Algorithms

**Problem:** Different layout strategies (vertical, horizontal, grid) should be interchangeable.

```rust
// src/domain/layout/

pub trait LayoutStrategy {
    fn arrange(&self, children: &mut [Box<dyn Widget>], bounds: Rect);
}

pub struct VerticalLayoutStrategy {
    spacing: f32,
}

impl LayoutStrategy for VerticalLayoutStrategy {
    fn arrange(&self, children: &mut [Box<dyn Widget>], bounds: Rect) {
        let mut y = bounds.top;
        let child_height = (bounds.height - self.spacing * (children.len() as f32 - 1.0)) / children.len() as f32;
        
        for child in children {
            child.arrange(Rect {
                left: bounds.left,
                top: y,
                right: bounds.right,
                bottom: y + child_height,
            });
            y += child_height + self.spacing;
        }
    }
}

pub struct GridLayoutStrategy {
    columns: usize,
    spacing: f32,
}

impl LayoutStrategy for GridLayoutStrategy {
    fn arrange(&self, children: &mut [Box<dyn Widget>], bounds: Rect) {
        // Grid layout implementation
    }
}

pub struct Container {
    children: Vec<Box<dyn Widget>>,
    layout_strategy: Box<dyn LayoutStrategy>,
}

impl Container {
    pub fn set_layout_strategy(&mut self, strategy: Box<dyn LayoutStrategy>) {
        self.layout_strategy = strategy;
    }
    
    pub fn arrange(&mut self, bounds: Rect) {
        self.layout_strategy.arrange(&mut self.children, bounds);
    }
}
```

#### **Template Method Pattern** - Widget Rendering Pipeline

**Problem:** All widgets follow the same rendering steps (setup, draw background, draw content, draw border), but details differ.

```rust
// src/domain/widgets/

pub trait WidgetTemplate {
    // Template method (defines algorithm structure)
    fn render(&self, renderer: &mut dyn RenderBackend) {
        self.before_render(renderer);
        self.render_background(renderer);
        self.render_content(renderer);
        self.render_border(renderer);
        self.after_render(renderer);
    }
    
    // Hooks (can be overridden by subclasses)
    fn before_render(&self, _renderer: &mut dyn RenderBackend) {}
    fn after_render(&self, _renderer: &mut dyn RenderBackend) {}
    
    // Abstract methods (must be implemented)
    fn render_background(&self, renderer: &mut dyn RenderBackend);
    fn render_content(&self, renderer: &mut dyn RenderBackend);
    fn render_border(&self, renderer: &mut dyn RenderBackend);
}

pub struct Textbox {
    // ...
}

impl WidgetTemplate for Textbox {
    fn render_background(&self, renderer: &mut dyn RenderBackend) {
        renderer.draw_rect(self.bounds, self.background_color);
    }
    
    fn render_content(&self, renderer: &mut dyn RenderBackend) {
        renderer.draw_text(&self.text, self.position, self.style);
    }
    
    fn render_border(&self, renderer: &mut dyn RenderBackend) {
        if self.border_width > 0.0 {
            renderer.draw_rounded_rect_outline(self.bounds, self.border_radius, self.border_color, self.border_width);
        }
    }
}
```

#### **Visitor Pattern** - Widget Tree Operations

**Problem:** Need to perform different operations on widget tree (measure, render, search) without modifying widget classes.

```rust
// src/domain/visitors/

pub trait WidgetVisitor {
    fn visit_textbox(&mut self, textbox: &Textbox);
    fn visit_listview(&mut self, listview: &ListView);
    fn visit_container(&mut self, container: &Container);
    // ... for each widget type
}

pub trait Visitable {
    fn accept(&self, visitor: &mut dyn WidgetVisitor);
}

impl Visitable for Textbox {
    fn accept(&self, visitor: &mut dyn WidgetVisitor) {
        visitor.visit_textbox(self);
    }
}

impl Visitable for Container {
    fn accept(&self, visitor: &mut dyn WidgetVisitor) {
        visitor.visit_container(self);
        for child in &self.children {
            child.accept(visitor);
        }
    }
}

// Example: Collect all text from widgets
pub struct TextCollectorVisitor {
    collected_text: String,
}

impl WidgetVisitor for TextCollectorVisitor {
    fn visit_textbox(&mut self, textbox: &Textbox) {
        self.collected_text.push_str(&textbox.text);
        self.collected_text.push('\n');
    }
    
    fn visit_listview(&mut self, listview: &ListView) {
        for item in listview.items() {
            self.collected_text.push_str(&item.text);
            self.collected_text.push('\n');
        }
    }
    
    fn visit_container(&mut self, _container: &Container) {
        // Traversal handled by Container's accept()
    }
}
```

---

## 3. SOLID Principles Application

### 3.1 Single Responsibility Principle (SRP)

**Current Violation:** `app.rs` (3,808 lines) handles:
- Window management
- Event handling
- Rendering coordination
- Application discovery
- Search logic
- History tracking
- Theme management
- Animation
- Task running
- Terminal emulation

**Refactored:**

```rust
// Each class has ONE reason to change

// src/application/services/search_service.rs
pub struct SearchService {
    // ONLY handles search algorithm
    fn search(&self, items: &[AppItem], query: &str) -> Vec<ScoredResult>;
}

// src/application/services/application_launcher.rs
pub struct ApplicationLauncher {
    // ONLY handles launching applications
    fn launch(&self, app: &AppItem) -> Result<(), LaunchError>;
}

// src/adapters/controllers/window_controller.rs
pub struct WindowController {
    // ONLY handles window events
    fn handle_message(&mut self, msg: WindowMessage);
}

// src/infrastructure/rendering/scene_renderer.rs
pub struct SceneRenderer {
    // ONLY handles rendering
    fn render(&mut self, scene: &Scene);
}

// src/domain/services/history_service.rs
pub struct HistoryService {
    // ONLY handles usage history
    fn record_launch(&mut self, app_id: &str);
    fn get_frequency(&self, app_id: &str) -> f32;
}
```

---

### 3.2 Open/Closed Principle (OCP)

**Principle:** Open for extension, closed for modification.

**Example 1: Widget System**

```rust
// Base trait (closed for modification)
pub trait Widget {
    fn render(&self, renderer: &mut dyn RenderBackend);
    fn handle_event(&mut self, event: &Event) -> EventResult;
}

// Extend by creating new widget types (open for extension)
pub struct CustomWidget { /* ... */ }

impl Widget for CustomWidget {
    fn render(&self, renderer: &mut dyn RenderBackend) {
        // Custom implementation
    }
    
    fn handle_event(&mut self, event: &Event) -> EventResult {
        // Custom implementation
        EventResult::Handled
    }
}

// No modification to existing widget code required!
```

**Example 2: Plugin System**

```rust
// src/domain/plugins/

pub trait Plugin {
    fn name(&self) -> &str;
    fn initialize(&mut self, context: &mut PluginContext) -> Result<(), PluginError>;
    fn register_commands(&self, registry: &mut CommandRegistry);
    fn register_widgets(&self, factory: &mut WidgetFactory);
}

pub struct PluginManager {
    plugins: Vec<Box<dyn Plugin>>,
}

impl PluginManager {
    pub fn register_plugin(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.push(plugin);
    }
    
    pub fn initialize_all(&mut self, context: &mut PluginContext) -> Result<(), PluginError> {
        for plugin in &mut self.plugins {
            plugin.initialize(context)?;
        }
        Ok(())
    }
}

// User creates custom plugin without modifying core
pub struct MyCustomPlugin;

impl Plugin for MyCustomPlugin {
    fn name(&self) -> &str { "my_custom_plugin" }
    
    fn initialize(&mut self, context: &mut PluginContext) -> Result<(), PluginError> {
        context.log("MyCustomPlugin initialized");
        Ok(())
    }
    
    fn register_commands(&self, registry: &mut CommandRegistry) {
        registry.register("custom_command", Box::new(MyCustomCommand));
    }
    
    fn register_widgets(&self, factory: &mut WidgetFactory) {
        factory.register("custom_widget", |config| Box::new(MyCustomWidget::new(config)));
    }
}
```

---

### 3.3 Liskov Substitution Principle (LSP)

**Principle:** Subtypes must be substitutable for their base types.

**Example:**

```rust
// Base trait with clear contract
pub trait RenderBackend {
    /// Draw a rectangle. MUST NOT throw if color is valid.
    /// Precondition: color values are in range 0.0-1.0
    /// Postcondition: rectangle is drawn at specified position
    fn draw_rect(&mut self, rect: Rect, color: Color);
}

// Both implementations respect the contract
impl RenderBackend for D2DRenderBackend {
    fn draw_rect(&mut self, rect: Rect, color: Color) {
        assert!(color.r >= 0.0 && color.r <= 1.0);
        assert!(color.g >= 0.0 && color.g <= 1.0);
        assert!(color.b >= 0.0 && color.b <= 1.0);
        assert!(color.a >= 0.0 && color.a <= 1.0);
        
        // Draw with Direct2D
    }
}

impl RenderBackend for SkiaRenderBackend {
    fn draw_rect(&mut self, rect: Rect, color: Color) {
        assert!(color.r >= 0.0 && color.r <= 1.0);
        // Same preconditions respected
        
        // Draw with Skia
    }
}

// Client code works with either implementation
fn render_ui(backend: &mut dyn RenderBackend) {
    backend.draw_rect(Rect::new(0.0, 0.0, 100.0, 100.0), Color::RED);
    // Guaranteed to work with any RenderBackend
}
```

**Anti-example (violates LSP):**

```rust
// BAD: Subtypes have different preconditions
pub trait AppRepository {
    fn search(&self, query: &str) -> Vec<AppItem>;
}

pub struct FileSystemAppRepository;

impl AppRepository for FileSystemAppRepository {
    fn search(&self, query: &str) -> Vec<AppItem> {
        // Works fine
    }
}

pub struct DatabaseAppRepository;

impl AppRepository for DatabaseAppRepository {
    fn search(&self, query: &str) -> Vec<AppItem> {
        if query.is_empty() {
            panic!("Query cannot be empty!");  // VIOLATION! Base doesn't require this
        }
        // ...
    }
}
```

---

### 3.4 Interface Segregation Principle (ISP)

**Principle:** Clients should not depend on interfaces they don't use.

**Current Violation:** Large `Widget` trait forces all widgets to implement methods they don't need.

**Refactored:**

```rust
// Split into focused interfaces

pub trait Renderable {
    fn render(&self, renderer: &mut dyn RenderBackend);
}

pub trait EventHandler {
    fn handle_event(&mut self, event: &Event) -> EventResult;
}

pub trait Layoutable {
    fn measure(&self, constraints: Constraints) -> Size;
    fn arrange(&mut self, bounds: Rect);
}

pub trait Focusable {
    fn focus(&mut self);
    fn blur(&mut self);
    fn is_focused(&self) -> bool;
}

pub trait Scrollable {
    fn scroll(&mut self, delta: f32);
    fn scroll_to(&mut self, position: f32);
}

// Widgets implement only what they need
pub struct Label {
    text: String,
    bounds: Rect,
}

impl Renderable for Label {
    fn render(&self, renderer: &mut dyn RenderBackend) {
        renderer.draw_text(&self.text, self.bounds.top_left(), TextStyle::default());
    }
}

impl Layoutable for Label {
    fn measure(&self, _constraints: Constraints) -> Size {
        Size::new(self.text.len() as f32 * 8.0, 20.0)
    }
    
    fn arrange(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
}

// Label doesn't implement EventHandler, Focusable, or Scrollable
// because it doesn't need those behaviors

pub struct Textbox {
    // ...
}

// Textbox implements everything
impl Renderable for Textbox { /* ... */ }
impl EventHandler for Textbox { /* ... */ }
impl Layoutable for Textbox { /* ... */ }
impl Focusable for Textbox { /* ... */ }

pub struct ListView {
    // ...
}

// ListView implements rendering, layout, events, and scrolling
impl Renderable for ListView { /* ... */ }
impl EventHandler for ListView { /* ... */ }
impl Layoutable for ListView { /* ... */ }
impl Scrollable for ListView { /* ... */ }
```

**Repository Segregation:**

```rust
// Instead of one fat AppRepository
pub trait AppRepository {
    fn discover_all(&self) -> Vec<AppItem>;
    fn search(&self, query: &str) -> Vec<AppItem>;
    fn get_icon(&self, path: &Path) -> Icon;
    fn launch(&self, app: &AppItem);
    fn get_history(&self) -> Vec<Launch>;
    fn save_history(&mut self, launch: Launch);
}

// Split into focused interfaces
pub trait AppDiscovery {
    fn discover_all(&self) -> Vec<AppItem>;
}

pub trait AppSearch {
    fn search(&self, query: &str) -> Vec<AppItem>;
}

pub trait IconProvider {
    fn get_icon(&self, path: &Path) -> Icon;
}

pub trait AppLauncher {
    fn launch(&self, app: &AppItem);
}

pub trait LaunchHistory {
    fn get_history(&self) -> Vec<Launch>;
    fn save_history(&mut self, launch: Launch);
}

// Clients depend only on what they need
pub struct SearchUseCase<D: AppDiscovery, S: AppSearch> {
    discovery: D,
    search: S,
}
```

---

### 3.5 Dependency Inversion Principle (DIP)

**Principle:** Depend on abstractions, not concretions. High-level modules should not depend on low-level modules.

**Current Violation:** App directly uses Win32 APIs, Direct2D, file system.

**Refactored:**

```rust
// High-level module (application use case)
pub struct LaunchApplicationUseCase {
    // Depends on abstraction (port)
    launcher: Arc<dyn AppLauncher>,
    history: Arc<dyn LaunchHistory>,
}

impl LaunchApplicationUseCase {
    pub fn execute(&mut self, app_id: &str) -> Result<(), Error> {
        // High-level business logic
        let app = self.find_app(app_id)?;
        self.launcher.launch(&app)?;
        self.history.save_history(Launch::new(app_id));
        Ok(())
    }
}

// Low-level module (infrastructure)
pub struct Win32AppLauncher {
    // Implementation details
}

impl AppLauncher for Win32AppLauncher {
    fn launch(&self, app: &AppItem) -> Result<(), Error> {
        // Win32-specific code
        unsafe {
            ShellExecuteW(/* ... */);
        }
        Ok(())
    }
}

// Dependency injection wires them together
let launcher: Arc<dyn AppLauncher> = Arc::new(Win32AppLauncher::new());
let history: Arc<dyn LaunchHistory> = Arc::new(FileHistoryGateway::new());
let use_case = LaunchApplicationUseCase::new(launcher, history);
```

**Visualization:**

```
WITHOUT DIP (bad):
┌─────────────────────┐
│   Application       │ depends on
│   (high-level)      │ ────────────┐
└─────────────────────┘             │
                                    ▼
                      ┌─────────────────────┐
                      │   Win32 API         │
                      │   (low-level)       │
                      └─────────────────────┘

WITH DIP (good):
┌─────────────────────┐              ┌─────────────────────┐
│   Application       │ depends on   │   Ports             │
│   (high-level)      │ ───────────> │   (abstraction)     │
└─────────────────────┘              └─────────────────────┘
                                               ▲
                                               │ implements
                                               │
                                     ┌─────────────────────┐
                                     │   Win32 Adapter     │
                                     │   (low-level)       │
                                     └─────────────────────┘
```

---

## 4. Migration Strategy

### Phase 1: Establish Foundations (Week 1-2)

**Goals:**
- Create layer structure
- Define core domain entities
- Extract domain logic

**Tasks:**
1. Create directory structure:
   ```
   src/
     domain/
       entities/
       value_objects/
       repositories/
       services/
     application/
       use_cases/
       ports/
       services/
     adapters/
       controllers/
       presenters/
       gateways/
       views/
     infrastructure/
       win32/
       filesystem/
       animation/
       parser/
   ```

2. Extract domain entities:
   - Move `AppItem`, `Theme`, etc. to `domain/entities/`
   - Move `Color`, `Rect`, `Size` to `domain/value_objects/`
   - Make them framework-agnostic

3. Define repository interfaces in `domain/repositories/`

4. Write unit tests for domain layer (zero Windows dependencies)

**Acceptance Criteria:**
- [ ] Domain layer compiles without Windows features
- [ ] 100% test coverage of domain logic
- [ ] No circular dependencies

---

### Phase 2: Application Layer (Week 3-4)

**Goals:**
- Extract use cases from monolithic App
- Define ports (interfaces)
- Create application services

**Tasks:**
1. Identify use cases from `app.rs`:
   - LaunchApplicationUseCase
   - SearchApplicationsUseCase
   - SwitchThemeUseCase
   - AnimateWindowUseCase
   - etc.

2. Define ports in `application/ports/`:
   - RenderPort
   - RuntimePort
   - AnimationPort
   - FileSystemPort

3. Create application services:
   - ApplicationCommandHandler
   - ThemeManager
   - WindowManager

4. Write integration tests (with mock implementations of ports)

**Acceptance Criteria:**
- [ ] All use cases extracted and tested
- [ ] Clear port interfaces defined
- [ ] Application layer has no direct Win32 dependencies

---

### Phase 3: Adapters Layer (Week 5-6)

**Goals:**
- Create controllers for input handling
- Create presenters for output formatting
- Implement gateways (repository implementations)

**Tasks:**
1. Create controllers:
   - WindowController (translates Win32 messages)
   - SearchController

2. Create presenters:
   - SearchPresenter
   - ThemePresenter

3. Implement gateways:
   - Win32AppGateway (AppRepository)
   - RasiThemeGateway (ThemeRepository)
   - FileHistoryGateway (HistoryRepository)

4. Define view interfaces

**Acceptance Criteria:**
- [ ] All Win32 message handling extracted to controllers
- [ ] Data transformation logic in presenters
- [ ] Repository interfaces fully implemented

---

### Phase 4: Infrastructure Layer (Week 7-8)

**Goals:**
- Implement all ports with concrete Win32/Direct2D code
- Create composition root for dependency injection
- Wire everything together

**Tasks:**
1. Implement rendering port:
   - D2DRenderBackend
   - Create abstraction over Direct2D

2. Implement other ports:
   - Win32Runtime
   - WindowAnimator
   - Win32FileSystem

3. Create `CompositionRoot`:
   - Instantiate all dependencies
   - Wire use cases, services, controllers
   - Provide factory methods

4. Update `main.rs`:
   - Create CompositionRoot
   - Start message loop
   - Handle errors gracefully

**Acceptance Criteria:**
- [ ] All ports implemented
- [ ] Dependency injection working
- [ ] Application runs end-to-end

---

### Phase 5: Apply Design Patterns (Week 9-10)

**Goals:**
- Refactor to use appropriate GoF patterns
- Improve extensibility and maintainability

**Tasks:**
1. Apply creational patterns:
   - Abstract Factory for widget creation
   - Builder for complex configurations
   - Flyweight for icon caching

2. Apply structural patterns:
   - Composite for widget hierarchy
   - Decorator for widget enhancements
   - Facade for platform APIs
   - Proxy for lazy loading

3. Apply behavioral patterns:
   - Command for actions/undo
   - Observer for events
   - State for window lifecycle
   - Strategy for layouts
   - Template Method for rendering pipeline

**Acceptance Criteria:**
- [ ] Patterns documented in code
- [ ] Code is more extensible
- [ ] No regression in functionality

---

### Phase 6: Refinement & Documentation (Week 11-12)

**Goals:**
- Optimize performance
- Complete documentation
- Add comprehensive tests

**Tasks:**
1. Performance profiling:
   - Identify bottlenecks
   - Optimize hot paths
   - Reduce allocations

2. Documentation:
   - Update AGENTS.md
   - Create architecture diagrams
   - Write API documentation
   - Add code examples

3. Testing:
   - Unit tests for all domain logic
   - Integration tests for use cases
   - End-to-end tests for critical paths
   - Property-based tests where applicable

**Acceptance Criteria:**
- [ ] No performance regressions
- [ ] >80% code coverage
- [ ] Complete architecture documentation
- [ ] All public APIs documented

---

## 5. File Organization

### New Directory Structure

```
wolfy/
├── src/
│   ├── domain/                      # Pure business logic
│   │   ├── entities/
│   │   │   ├── app_item.rs
│   │   │   ├── theme.rs
│   │   │   ├── window_state.rs
│   │   │   └── task.rs
│   │   ├── value_objects/
│   │   │   ├── color.rs
│   │   │   ├── rect.rs
│   │   │   ├── dimensions.rs
│   │   │   ├── hotkey.rs
│   │   │   └── search_query.rs
│   │   ├── repositories/           # Interfaces only
│   │   │   ├── app_repository.rs
│   │   │   ├── theme_repository.rs
│   │   │   ├── icon_repository.rs
│   │   │   └── history_repository.rs
│   │   ├── services/               # Domain services
│   │   │   ├── search_service.rs
│   │   │   ├── theme_resolver.rs
│   │   │   └── fuzzy_matcher.rs
│   │   ├── errors.rs
│   │   └── mod.rs
│   │
│   ├── application/                 # Use cases & orchestration
│   │   ├── use_cases/
│   │   │   ├── launch_app.rs
│   │   │   ├── search_apps.rs
│   │   │   ├── switch_theme.rs
│   │   │   ├── animate_window.rs
│   │   │   ├── show_window.rs
│   │   │   └── run_task.rs
│   │   ├── ports/                  # Port interfaces
│   │   │   ├── render_port.rs
│   │   │   ├── runtime_port.rs
│   │   │   ├── animation_port.rs
│   │   │   ├── filesystem_port.rs
│   │   │   └── input_port.rs
│   │   ├── services/               # Application services
│   │   │   ├── command_handler.rs
│   │   │   ├── theme_manager.rs
│   │   │   ├── window_manager.rs
│   │   │   └── task_manager.rs
│   │   ├── dto/                    # Data transfer objects
│   │   │   ├── app_item_dto.rs
│   │   │   ├── theme_dto.rs
│   │   │   └── window_state_dto.rs
│   │   └── mod.rs
│   │
│   ├── adapters/                    # Interface adapters
│   │   ├── controllers/
│   │   │   ├── window_controller.rs
│   │   │   ├── search_controller.rs
│   │   │   ├── hotkey_controller.rs
│   │   │   └── task_controller.rs
│   │   ├── presenters/
│   │   │   ├── search_presenter.rs
│   │   │   ├── theme_presenter.rs
│   │   │   └── task_presenter.rs
│   │   ├── gateways/               # Repository implementations
│   │   │   ├── win32_app_gateway.rs
│   │   │   ├── rasi_theme_gateway.rs
│   │   │   ├── file_history_gateway.rs
│   │   │   └── icon_cache_gateway.rs
│   │   ├── views/                  # View interfaces
│   │   │   ├── search_view.rs
│   │   │   ├── theme_view.rs
│   │   │   └── window_view.rs
│   │   └── mod.rs
│   │
│   ├── infrastructure/              # External frameworks & tools
│   │   ├── win32/
│   │   │   ├── window_impl.rs
│   │   │   ├── d2d_renderer.rs
│   │   │   ├── hotkey_impl.rs
│   │   │   ├── icon_loader.rs
│   │   │   ├── app_discovery.rs
│   │   │   ├── runtime.rs
│   │   │   └── mod.rs
│   │   ├── filesystem/
│   │   │   ├── file_watcher.rs
│   │   │   ├── config_loader.rs
│   │   │   └── mod.rs
│   │   ├── animation/
│   │   │   ├── animator.rs
│   │   │   ├── easing.rs
│   │   │   └── mod.rs
│   │   ├── parser/
│   │   │   ├── rasi_parser.rs
│   │   │   ├── lexer.rs
│   │   │   ├── theme.lalrpop
│   │   │   └── mod.rs
│   │   ├── composition_root.rs     # Dependency injection
│   │   └── mod.rs
│   │
│   ├── ui/                          # UI layer (widgets, rendering)
│   │   ├── widgets/
│   │   │   ├── base.rs
│   │   │   ├── textbox.rs
│   │   │   ├── listview.rs
│   │   │   ├── container.rs
│   │   │   ├── gridview.rs
│   │   │   ├── panel.rs
│   │   │   └── decorators/
│   │   │       ├── scrollable.rs
│   │   │       ├── border.rs
│   │   │       └── shadow.rs
│   │   ├── layout/
│   │   │   ├── engine.rs
│   │   │   ├── strategies/
│   │   │   │   ├── vertical.rs
│   │   │   │   ├── horizontal.rs
│   │   │   │   └── grid.rs
│   │   │   └── mod.rs
│   │   ├── rendering/
│   │   │   ├── scene.rs
│   │   │   ├── backend.rs
│   │   │   └── primitives.rs
│   │   └── mod.rs
│   │
│   ├── shared/                      # Shared utilities
│   │   ├── logging.rs
│   │   ├── config.rs
│   │   └── mod.rs
│   │
│   ├── lib.rs                       # Library exports
│   └── main.rs                      # Application entry point
│
├── tests/                           # Integration tests
│   ├── use_cases/
│   ├── adapters/
│   └── infrastructure/
│
├── Cargo.toml
├── AGENTS.md
├── ARCHITECTURE.md                  # This document
└── README.md
```

---

## 6. Testing Strategy

### 6.1 Domain Layer Tests (Pure Unit Tests)

```rust
// tests/domain/services/search_service_test.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fuzzy_match_exact() {
        let service = SearchService::new();
        let items = vec![
            AppItem::new("Chrome", "/path/to/chrome"),
            AppItem::new("Firefox", "/path/to/firefox"),
        ];
        
        let query = SearchQuery::new("Chrome");
        let results = service.search(&items, &query);
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].item.name, "Chrome");
        assert!(results[0].score > 100);
    }
    
    #[test]
    fn test_fuzzy_match_partial() {
        let service = SearchService::new();
        let items = vec![
            AppItem::new("Visual Studio Code", "/path/to/vscode"),
        ];
        
        let query = SearchQuery::new("vsc");
        let results = service.search(&items, &query);
        
        assert_eq!(results.len(), 1);
        assert!(results[0].score > 0);
    }
}
```

### 6.2 Application Layer Tests (Integration Tests with Mocks)

```rust
// tests/application/use_cases/launch_app_test.rs

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    
    // Mock runtime
    struct MockRuntime {
        launched: Arc<Mutex<Vec<String>>>,
    }
    
    impl RuntimePort for MockRuntime {
        fn execute(&self, path: &str) -> Result<(), RuntimeError> {
            self.launched.lock().unwrap().push(path.to_string());
            Ok(())
        }
    }
    
    #[test]
    fn test_launch_app_success() {
        let launched = Arc::new(Mutex::new(Vec::new()));
        let runtime = Arc::new(MockRuntime { launched: launched.clone() });
        let history = Arc::new(MockHistoryRepository::new());
        
        let use_case = LaunchApplicationUseCase::new(runtime, history);
        let app = AppItem::new("Test", "/path/to/test.exe");
        
        let result = use_case.execute(&app);
        
        assert!(result.is_ok());
        assert_eq!(launched.lock().unwrap()[0], "/path/to/test.exe");
    }
    
    #[test]
    fn test_launch_app_records_history() {
        let runtime = Arc::new(MockRuntime::new());
        let history = Arc::new(MockHistoryRepository::new());
        
        let use_case = LaunchApplicationUseCase::new(runtime, history.clone());
        let app = AppItem::new("Test", "/path/to/test.exe");
        
        use_case.execute(&app).unwrap();
        
        assert_eq!(history.get_launch_count("Test"), 1);
    }
}
```

### 6.3 Adapter Tests

```rust
// tests/adapters/gateways/win32_app_gateway_test.rs

#[cfg(test)]
#[cfg(windows)]
mod tests {
    use super::*;
    
    #[test]
    fn test_discover_all_finds_apps() {
        let gateway = Win32AppGateway::new();
        let apps = gateway.discover_all().unwrap();
        
        // Should find at least some apps on any Windows system
        assert!(apps.len() > 0);
        
        // Check first app has valid properties
        assert!(!apps[0].name.is_empty());
        assert!(apps[0].path.exists());
    }
}
```

### 6.4 Property-Based Tests

```rust
// tests/domain/value_objects/color_test.rs

use proptest::prelude::*;

proptest! {
    #[test]
    fn test_color_values_in_range(r in 0.0f32..=1.0, g in 0.0f32..=1.0, b in 0.0f32..=1.0, a in 0.0f32..=1.0) {
        let color = Color::new(r, g, b, a);
        
        prop_assert!(color.r >= 0.0 && color.r <= 1.0);
        prop_assert!(color.g >= 0.0 && color.g <= 1.0);
        prop_assert!(color.b >= 0.0 && color.b <= 1.0);
        prop_assert!(color.a >= 0.0 && color.a <= 1.0);
    }
    
    #[test]
    fn test_color_hex_roundtrip(r in 0u8..=255, g in 0u8..=255, b in 0u8..=255) {
        let original = Color::from_rgb(r, g, b);
        let hex = original.to_hex();
        let parsed = Color::from_hex(&hex).unwrap();
        
        prop_assert!((original.r - parsed.r).abs() < 0.01);
        prop_assert!((original.g - parsed.g).abs() < 0.01);
        prop_assert!((original.b - parsed.b).abs() < 0.01);
    }
}
```

---

## 7. Benefits of Refactoring

### 7.1 Maintainability
- **Single Responsibility:** Each module has one clear purpose
- **Low Coupling:** Changes in one layer don't affect others
- **High Cohesion:** Related functionality grouped together

### 7.2 Testability
- **Domain Layer:** 100% testable without Windows
- **Use Cases:** Testable with mock implementations
- **Clear Test Boundaries:** Each layer can be tested independently

### 7.3 Extensibility
- **Plugin System:** Easy to add new features
- **Multiple Backends:** Can support Linux/macOS
- **Custom Widgets:** Users can create their own
- **Theme System:** Extensible without code changes

### 7.4 Reusability
- **Domain Logic:** Can be used in CLI, web server, or GUI
- **Application Layer:** Framework-agnostic business logic
- **Widget System:** Reusable components

### 7.5 Performance
- **Lazy Loading:** Only load what's needed
- **Caching:** Flyweight pattern reduces memory
- **Async Operations:** Non-blocking architecture

### 7.6 Developer Experience
- **Clear Structure:** New developers understand quickly
- **Documentation:** Architecture is self-documenting
- **Error Messages:** Clear error types per layer
- **Debugging:** Easy to isolate issues

---

## 8. Risks and Mitigation

### Risk 1: Breaking Changes

**Mitigation:**
- Feature flag system for gradual rollout
- Comprehensive test suite before refactoring
- Keep old code paths until new ones proven
- Incremental migration, not big bang

### Risk 2: Performance Regression

**Mitigation:**
- Benchmark before and after
- Profile hot paths
- Optimize abstractions (zero-cost where possible)
- Use `dyn Trait` sparingly, prefer generics

### Risk 3: Over-Engineering

**Mitigation:**
- Apply patterns only where they add value
- Start simple, refactor when needed
- Measure complexity with metrics
- Regular code reviews

### Risk 4: Learning Curve

**Mitigation:**
- Comprehensive documentation
- Code examples for each pattern
- Pair programming sessions
- Architecture decision records (ADRs)

---

## 9. Success Metrics

### Code Quality
- [ ] Lines of code per file < 300
- [ ] Cyclomatic complexity < 10 per function
- [ ] Test coverage > 80%
- [ ] Zero clippy warnings

### Architecture
- [ ] Clear layer boundaries
- [ ] No circular dependencies
- [ ] All dependencies point inward (Clean Architecture)
- [ ] Dependency injection throughout

### Performance
- [ ] Startup time < 100ms
- [ ] Search latency < 16ms (60fps)
- [ ] Memory usage < 50MB
- [ ] No memory leaks

### Developer Metrics
- [ ] New feature development time reduced by 30%
- [ ] Bug fix time reduced by 40%
- [ ] Onboarding time for new developers < 1 day
- [ ] Architecture documentation complete

---

## 10. Conclusion

This refactoring plan transforms Wolfy from a monolithic, tightly-coupled application into a well-architected, maintainable, and extensible codebase following enterprise best practices.

**Key Takeaways:**
1. **Clean Architecture** provides clear separation of concerns
2. **GoF Patterns** solve specific recurring design problems
3. **SOLID Principles** ensure code quality and maintainability
4. **Incremental Migration** reduces risk
5. **Comprehensive Testing** catches regressions early

**Next Steps:**
1. Review and approve this plan
2. Set up new directory structure
3. Begin Phase 1 (Domain Layer extraction)
4. Iterate through phases with regular checkpoints

**Timeline:** 12 weeks for complete refactoring

**Resources Needed:**
- 1-2 senior developers
- Code review process
- CI/CD pipeline for automated testing
- Documentation tools

---

## References

- **Clean Architecture** by Robert C. Martin
- **Design Patterns: Elements of Reusable Object-Oriented Software** by Gang of Four
- **Domain-Driven Design** by Eric Evans
- **Refactoring** by Martin Fowler
- **The Rust Programming Language** (for Rust-specific patterns)

---

*Generated: 2026-02-01*  
*Version: 1.0*  
*Status: Draft for Review*
