//! Windows application discovery
//!
//! Discovers installed applications from:
//! - Start Menu shortcuts (.lnk files)
//! - shell:AppsFolder (UWP/Store apps)

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    Common::ITEMIDLIST, FOLDERID_AppsFolder, IEnumIDList, IShellFolder, IShellItem,
    ILCombine, SHBindToObject, SHCreateItemFromIDList, SHGetDesktopFolder,
    SHGetKnownFolderIDList, SHCONTF_NONFOLDERS, SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_NORMALDISPLAY,
};

use super::shortcut::parse_lnk;
use crate::history::History;

/// Discovered application entry
#[derive(Debug, Clone)]
pub struct AppEntry {
    /// Unique identifier for history tracking
    pub id: String,
    /// Display name
    pub name: String,
    /// Description/subtext
    pub description: String,
    /// Path for icon extraction
    pub icon_source: String,
    /// Command to launch (path or shell:AppsFolder ID)
    pub launch_target: String,
    /// Whether this is a UWP/Store app
    pub is_uwp: bool,
}

/// Patterns to filter out from app names (case-insensitive)
const FILTER_NAME_PATTERNS: &[&str] = &[
    "uninstall",
    "remove",
    "setup",
    "install",
    "readme",
    "help",
    "documentation",
    "manual",
    "license",
    "changelog",
    "release notes",
    "what's new",
    "configuration wizard",
    "repair",
    "update",
];

/// Folder names to skip (case-insensitive)
const FILTER_FOLDER_PATTERNS: &[&str] = &["startup", "maintenance"];

/// Target executables to filter out
const FILTER_TARGETS: &[&str] = &["msiexec.exe", "control.exe", "rundll32.exe"];

/// Check if a name should be filtered out
fn should_filter_name(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    FILTER_NAME_PATTERNS
        .iter()
        .any(|pattern| name_lower.contains(pattern))
}

/// Check if a path is in a filtered folder
fn should_filter_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    FILTER_FOLDER_PATTERNS
        .iter()
        .any(|pattern| path_str.contains(pattern))
}

/// Check if a target should be filtered out
fn should_filter_target(target: &str) -> bool {
    let target_lower = target.to_lowercase();
    FILTER_TARGETS
        .iter()
        .any(|pattern| target_lower.ends_with(pattern))
}

/// Discover all applications from Start Menu and AppsFolder
///
/// # Arguments
/// * `history` - Optional history for sorting by frequency
///
/// # Returns
/// A vector of discovered applications, sorted by usage frequency then alphabetically
pub fn discover_all_apps(history: Option<&History>) -> Vec<AppEntry> {
    log!("Starting app discovery...");

    let mut apps = Vec::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    // Discover Start Menu apps
    let start_menu_apps = discover_start_menu_apps();
    log!("Found {} Start Menu apps", start_menu_apps.len());

    for app in start_menu_apps {
        let name_lower = app.name.to_lowercase();
        if !seen_names.contains(&name_lower) {
            seen_names.insert(name_lower);
            apps.push(app);
        }
    }

    // Discover UWP apps from AppsFolder
    let uwp_apps = discover_apps_folder();
    log!("Found {} AppsFolder apps", uwp_apps.len());

    for app in uwp_apps {
        let name_lower = app.name.to_lowercase();
        if !seen_names.contains(&name_lower) {
            seen_names.insert(name_lower);
            apps.push(app);
        }
    }

    // Sort by history (frequency) then alphabetically
    sort_apps(&mut apps, history);

    // Filter out WSLg apps (e.g., "App Name (Ubuntu)", "App Name (Arch Linux)")
    apps.retain(|app| !is_wslg_app(&app.name));

    log!("Total apps after dedup and sort: {}", apps.len());
    apps
}

/// Check if an app name indicates a WSLg (Windows Subsystem for Linux GUI) app
/// These typically have a Linux distro name in parentheses at the end
fn is_wslg_app(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    // Common WSL distro patterns
    name_lower.ends_with("(ubuntu)")
        || name_lower.ends_with("(ubuntu-22.04)")
        || name_lower.ends_with("(ubuntu-24.04)")
        || name_lower.ends_with("(debian)")
        || name_lower.ends_with("(arch)")
        || name_lower.ends_with("(archlinux)")
        || name_lower.ends_with("(arch linux)")
        || name_lower.ends_with("(fedora)")
        || name_lower.ends_with("(opensuse)")
        || name_lower.ends_with("(kali)")
        || name_lower.ends_with("(kali-linux)")
        || name_lower.contains("(ubuntu")
        || name_lower.contains("(debian")
        || name_lower.contains("(arch")
        || name_lower.contains("(fedora")
        || name_lower.contains("(opensuse")
        || name_lower.contains("(kali")
}

/// Sort apps by usage frequency (descending) then alphabetically
fn sort_apps(apps: &mut [AppEntry], history: Option<&History>) {
    apps.sort_by(|a, b| {
        match history {
            Some(h) => {
                let idx_a = h.sort_index(&a.id);
                let idx_b = h.sort_index(&b.id);

                // Both in history: sort by frequency (descending)
                if idx_a >= 0 && idx_b >= 0 {
                    return idx_b.cmp(&idx_a);
                }

                // One in history: history item first
                if idx_a >= 0 {
                    return std::cmp::Ordering::Less;
                }
                if idx_b >= 0 {
                    return std::cmp::Ordering::Greater;
                }

                // Neither in history: alphabetical
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            }
            None => {
                // No history: just alphabetical
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            }
        }
    });
}

/// Discover applications from Start Menu folders
fn discover_start_menu_apps() -> Vec<AppEntry> {
    let mut apps = Vec::new();

    // User Start Menu
    if let Some(appdata) = std::env::var_os("APPDATA") {
        let user_start_menu = PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        scan_start_menu_folder(&user_start_menu, &mut apps);
    }

    // All Users Start Menu
    if let Some(programdata) = std::env::var_os("PROGRAMDATA") {
        let common_start_menu = PathBuf::from(programdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        scan_start_menu_folder(&common_start_menu, &mut apps);
    }

    apps
}

/// Recursively scan a Start Menu folder for .lnk files
fn scan_start_menu_folder(folder: &Path, apps: &mut Vec<AppEntry>) {
    if !folder.exists() || !folder.is_dir() {
        return;
    }

    // Check if this folder should be skipped
    if should_filter_path(folder) {
        log!("Skipping filtered folder: {:?}", folder);
        return;
    }

    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(e) => {
            log!("Failed to read directory {:?}: {:?}", folder, e);
            return;
        }
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();

        if path.is_dir() {
            // Recurse into subdirectories
            scan_start_menu_folder(&path, apps);
        } else if path
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("lnk"))
        {
            // Parse .lnk file
            if let Some(app) = parse_shortcut(&path) {
                apps.push(app);
            }
        }
    }
}

/// Parse a single .lnk shortcut file into an AppEntry
fn parse_shortcut(lnk_path: &Path) -> Option<AppEntry> {
    // Get the shortcut name (filename without .lnk extension)
    let name = lnk_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    // Filter by name
    if should_filter_name(&name) {
        log!("Filtering out by name: {}", name);
        return None;
    }

    // Parse the shortcut
    let info = match parse_lnk(lnk_path) {
        Ok(info) => info,
        Err(e) => {
            log!("Failed to parse {:?}: {}", lnk_path, e);
            return None;
        }
    };

    // Filter by target
    if should_filter_target(&info.target_path) {
        log!("Filtering out by target: {} -> {}", name, info.target_path);
        return None;
    }

    // Validate target exists
    if !info.target_exists() {
        log!("Target doesn't exist: {} -> {}", name, info.target_path);
        return None;
    }

    // Use the .lnk path as the ID (unique identifier)
    let id = lnk_path.to_string_lossy().to_string();

    // Use shortcut description if available, otherwise empty
    let description = if !info.description.is_empty() {
        info.description.clone()
    } else {
        String::new()
    };

    // Icon source: prefer the shortcut's icon, fall back to target
    let icon_source = info.icon_source().to_string();

    // Launch target: use the .lnk path itself (Windows will resolve it)
    let launch_target = lnk_path.to_string_lossy().to_string();

    Some(AppEntry {
        id,
        name,
        description,
        icon_source,
        launch_target,
        is_uwp: false,
    })
}

/// Discover UWP/Store apps from shell:AppsFolder
fn discover_apps_folder() -> Vec<AppEntry> {
    let mut apps = Vec::new();

    unsafe {
        // Initialize COM
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        // Get the PIDL for AppsFolder
        let pidl = match SHGetKnownFolderIDList(&FOLDERID_AppsFolder, Default::default(), None) {
            Ok(pidl) => pidl,
            Err(e) => {
                log!("Failed to get AppsFolder PIDL: {:?}", e);
                return apps;
            }
        };

        // Get desktop folder
        let desktop: IShellFolder = match SHGetDesktopFolder() {
            Ok(d) => d,
            Err(e) => {
                log!("Failed to get desktop folder: {:?}", e);
                return apps;
            }
        };

        // Bind to AppsFolder
        let apps_folder: IShellFolder = match SHBindToObject(&desktop, pidl, None) {
            Ok(f) => f,
            Err(e) => {
                log!("Failed to bind to AppsFolder: {:?}", e);
                return apps;
            }
        };

        // Enumerate items
        let mut enum_list: Option<IEnumIDList> = None;
        let hr =
            apps_folder.EnumObjects(HWND::default(), SHCONTF_NONFOLDERS.0 as u32, &mut enum_list);
        if hr.is_err() {
            log!("Failed to enumerate AppsFolder: {:?}", hr);
            return apps;
        }
        if enum_list.is_none() {
            return apps;
        }
        let enum_list = enum_list.unwrap();

        // Iterate through items
        loop {
            let mut pidl_array: [*mut ITEMIDLIST; 1] = [std::ptr::null_mut(); 1];
            let mut fetched: u32 = 0;

            if enum_list.Next(&mut pidl_array, Some(&mut fetched)).is_err() || fetched == 0 {
                break;
            }

            let child_pidl = pidl_array[0];
            if child_pidl.is_null() {
                break;
            }

            // Combine parent (AppsFolder) and child PIDLs to get absolute PIDL
            let absolute_pidl = ILCombine(Some(pidl), Some(child_pidl));
            if absolute_pidl.is_null() {
                windows::Win32::System::Com::CoTaskMemFree(Some(child_pidl as *const _));
                continue;
            }

            // Try to get display name via IShellItem using absolute PIDL
            if let Ok(shell_item) = SHCreateItemFromIDList::<IShellItem>(absolute_pidl) {
                if let Ok(display_name_ptr) = shell_item.GetDisplayName(SIGDN_NORMALDISPLAY) {
                    let name = pwstr_to_string(display_name_ptr);

                    // Free the display name string
                    windows::Win32::System::Com::CoTaskMemFree(Some(
                        display_name_ptr.0 as *const _,
                    ));

                    // Skip filtered names
                    if !should_filter_name(&name) && !name.is_empty() {
                        // Get the AUMID (App User Model ID) for launching
                        let launch_target =
                            if let Ok(aumid_ptr) = shell_item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING) {
                                let aumid = pwstr_to_string(aumid_ptr);
                                windows::Win32::System::Com::CoTaskMemFree(Some(
                                    aumid_ptr.0 as *const _,
                                ));
                                // Format as shell:AppsFolder path for explorer.exe to launch
                                format!("shell:AppsFolder\\{}", aumid)
                            } else {
                                name.clone()
                            };

                        apps.push(AppEntry {
                            id: format!("uwp:{}", name),
                            name: name.clone(),
                            description: String::new(),
                            icon_source: String::new(),
                            launch_target,
                            is_uwp: true,
                        });
                    }
                }
            }

            // Free the PIDLs
            windows::Win32::System::Com::CoTaskMemFree(Some(absolute_pidl as *const _));
            windows::Win32::System::Com::CoTaskMemFree(Some(child_pidl as *const _));
        }
    }

    apps
}

/// Convert a PWSTR to a Rust String
fn pwstr_to_string(pwstr: windows::core::PWSTR) -> String {
    if pwstr.is_null() {
        return String::new();
    }

    unsafe {
        let len = (0..).take_while(|&i| *pwstr.0.add(i) != 0).count();
        let slice = std::slice::from_raw_parts(pwstr.0, len);
        String::from_utf16_lossy(slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_filter_name() {
        assert!(should_filter_name("Uninstall Firefox"));
        assert!(should_filter_name("Remove Application"));
        assert!(should_filter_name("Setup Wizard"));
        assert!(should_filter_name("readme"));
        assert!(should_filter_name("LICENSE"));

        assert!(!should_filter_name("Firefox"));
        assert!(!should_filter_name("Visual Studio Code"));
        assert!(!should_filter_name("Notepad"));
    }

    #[test]
    fn test_should_filter_target() {
        assert!(should_filter_target("C:\\Windows\\System32\\msiexec.exe"));
        assert!(should_filter_target("mmc.exe"));
        assert!(should_filter_target("rundll32.exe"));

        assert!(!should_filter_target("C:\\Program Files\\App\\app.exe"));
        assert!(!should_filter_target("notepad.exe"));
    }

    #[test]
    fn test_should_filter_path() {
        assert!(should_filter_path(Path::new(
            "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Administrative Tools"
        )));
        assert!(should_filter_path(Path::new(
            "C:\\Users\\Test\\AppData\\Roaming\\Microsoft\\Windows\\Start Menu\\Programs\\Accessibility"
        )));

        assert!(!should_filter_path(Path::new(
            "C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Firefox"
        )));
    }
}
