//! PR Reviews tracking module
//!
//! Tracks PR review markdown files and their viewed state.
//! Files are scanned from a configured directory matching pattern: PR-{number}-{date}.md
//!
//! Configuration is loaded from extensions.toml:
//! ```toml
//! [pr_reviews]
//! enabled = true
//! reviews_dir = "C:\\path\\to\\reviews"
//! ```

use chrono::{Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

// ============================================================================
// EXTENSION CONFIGURATION (loaded from extensions.toml)
// ============================================================================

/// Extensions configuration loaded from extensions.toml
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ExtensionsConfig {
    /// PR Reviews extension configuration
    #[serde(default)]
    pub pr_reviews: PrReviewsExtConfig,
}

/// PR Reviews extension configuration from extensions.toml
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PrReviewsExtConfig {
    /// Whether the extension is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Directory containing PR review markdown files
    #[serde(default)]
    pub reviews_dir: Option<PathBuf>,
}

impl Default for PrReviewsExtConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            reviews_dir: None,
        }
    }
}

fn default_true() -> bool {
    true
}

impl ExtensionsConfig {
    /// Find extensions.toml in standard locations
    fn find_config_path() -> Option<PathBuf> {
        let candidates = [
            dirs::config_dir().map(|p| p.join("wolfy").join("extensions.toml")),
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("extensions.toml"))),
            Some(PathBuf::from("extensions.toml")),
        ];

        for candidate in candidates.into_iter().flatten() {
            if candidate.exists() {
                return Some(candidate);
            }
        }
        None
    }

    /// Load configuration from file, returning defaults if not found
    fn load() -> Self {
        if let Some(path) = Self::find_config_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str(&content) {
                    return config;
                }
            }
        }
        Self::default()
    }
}

// ============================================================================
// PR REVIEWS CONFIGURATION
// ============================================================================

/// Configuration for PR reviews
#[derive(Debug, Clone)]
pub struct PrReviewsConfig {
    /// Whether PR reviews feature is enabled
    pub enabled: bool,
    /// Directory containing PR review markdown files
    pub reviews_dir: PathBuf,
}

impl Default for PrReviewsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            reviews_dir: PathBuf::from(r"C:\Users\fevargas\Source\MSFT\PRReviewer\pr-reports"),
        }
    }
}

impl PrReviewsConfig {
    /// Load configuration from extensions.toml, falling back to defaults
    pub fn load() -> Self {
        let ext_config = ExtensionsConfig::load();
        let default = Self::default();
        Self {
            enabled: ext_config.pr_reviews.enabled,
            reviews_dir: ext_config.pr_reviews.reviews_dir.unwrap_or(default.reviews_dir),
        }
    }
}

/// A single PR review file
#[derive(Debug, Clone)]
pub struct PrReview {
    /// Full path to the markdown file
    pub path: PathBuf,
    /// PR number extracted from filename
    pub pr_number: u32,
    /// Date from filename
    pub date: NaiveDate,
    /// Whether this review has been viewed
    pub viewed: bool,
    /// Cached content (loaded on demand)
    pub content: Option<String>,
}

impl PrReview {
    /// Get the filename for display
    pub fn filename(&self) -> String {
        self.path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    /// Load content from file
    pub fn load_content(&mut self) -> Result<&str, std::io::Error> {
        if self.content.is_none() {
            let content = fs::read_to_string(&self.path)?;
            self.content = Some(content);
        }
        Ok(self.content.as_ref().unwrap())
    }

    /// Get a short title (first line or PR number)
    pub fn title(&self) -> String {
        if let Some(ref content) = self.content {
            // Find first heading or use first line
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("# ") {
                    return trimmed.trim_start_matches("# ").to_string();
                }
                if !trimmed.is_empty() {
                    return trimmed.chars().take(50).collect();
                }
            }
        }
        format!("PR #{}", self.pr_number)
    }
}

/// Persistent state for viewed reviews
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ViewedState {
    /// Set of viewed file paths (stored as strings for serialization)
    viewed_files: HashSet<String>,
}

/// PR Reviews state manager
#[derive(Debug, Clone)]
pub struct PrReviews {
    /// Configuration
    config: PrReviewsConfig,
    /// Today's reviews
    reviews: Vec<PrReview>,
    /// Path to state file
    state_path: PathBuf,
}

impl Default for PrReviews {
    fn default() -> Self {
        Self::new(PrReviewsConfig::default())
    }
}

impl PrReviews {
    /// Create a new PR reviews manager
    pub fn new(config: PrReviewsConfig) -> Self {
        let state_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wolfy")
            .join("pr_reviews_viewed.json");

        let mut manager = Self {
            config,
            reviews: Vec::new(),
            state_path,
        };
        manager.refresh();
        manager
    }

    /// Create with configuration loaded from extensions.toml
    pub fn new_default() -> Self {
        Self::new(PrReviewsConfig::load())
    }

    /// Check if the PR reviews extension is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Refresh the list of today's reviews
    pub fn refresh(&mut self) {
        let today = Local::now().date_naive();
        self.reviews = self.scan_reviews_for_date(today);
        self.load_viewed_state();
    }

    /// Scan directory for reviews matching a specific date
    fn scan_reviews_for_date(&self, date: NaiveDate) -> Vec<PrReview> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let mut reviews = Vec::new();

        if !self.config.reviews_dir.exists() {
            return reviews;
        }

        if let Ok(entries) = fs::read_dir(&self.config.reviews_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if let Some(review) = self.parse_review_file(&path, &date_str) {
                    reviews.push(review);
                }
            }
        }

        // Sort by PR number
        reviews.sort_by_key(|r| r.pr_number);
        reviews
    }

    /// Parse a review file if it matches the expected pattern
    fn parse_review_file(&self, path: &Path, date_str: &str) -> Option<PrReview> {
        let filename = path.file_name()?.to_str()?;

        // Expected format: PR-{number}-{date}.md
        if !filename.starts_with("PR-") || !filename.ends_with(".md") {
            return None;
        }

        // Check if date matches
        if !filename.contains(date_str) {
            return None;
        }

        // Extract PR number: PR-{number}-{date}.md
        let parts: Vec<&str> = filename.trim_end_matches(".md").split('-').collect();
        if parts.len() < 4 {
            return None;
        }

        let pr_number: u32 = parts[1].parse().ok()?;

        // Parse date from filename (last 3 parts: YYYY-MM-DD)
        let date_parts = &parts[parts.len() - 3..];
        let date = NaiveDate::parse_from_str(
            &format!("{}-{}-{}", date_parts[0], date_parts[1], date_parts[2]),
            "%Y-%m-%d",
        )
        .ok()?;

        Some(PrReview {
            path: path.to_path_buf(),
            pr_number,
            date,
            viewed: false,
            content: None,
        })
    }

    /// Load viewed state from disk
    fn load_viewed_state(&mut self) {
        if let Ok(content) = fs::read_to_string(&self.state_path) {
            if let Ok(state) = serde_json::from_str::<ViewedState>(&content) {
                for review in &mut self.reviews {
                    let path_str = review.path.to_string_lossy().to_string();
                    review.viewed = state.viewed_files.contains(&path_str);
                }
            }
        }
    }

    /// Save viewed state to disk
    fn save_viewed_state(&self) {
        let state = ViewedState {
            viewed_files: self
                .reviews
                .iter()
                .filter(|r| r.viewed)
                .map(|r| r.path.to_string_lossy().to_string())
                .collect(),
        };

        // Ensure directory exists
        if let Some(parent) = self.state_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(json) = serde_json::to_string_pretty(&state) {
            let _ = fs::write(&self.state_path, json);
        }
    }

    /// Get count of unread reviews
    pub fn unread_count(&self) -> usize {
        self.reviews.iter().filter(|r| !r.viewed).count()
    }

    /// Get total count of today's reviews
    pub fn total_count(&self) -> usize {
        self.reviews.len()
    }

    /// Check if there are any reviews today
    pub fn has_reviews(&self) -> bool {
        !self.reviews.is_empty()
    }

    /// Check if there are any unread reviews
    pub fn has_unread(&self) -> bool {
        self.unread_count() > 0
    }

    /// Get today's reviews
    pub fn reviews(&self) -> &[PrReview] {
        &self.reviews
    }

    /// Get a mutable reference to a review by index
    pub fn get_review_mut(&mut self, index: usize) -> Option<&mut PrReview> {
        self.reviews.get_mut(index)
    }

    /// Get a review by index
    pub fn get_review(&self, index: usize) -> Option<&PrReview> {
        self.reviews.get(index)
    }

    /// Mark a review as viewed by index and save
    pub fn mark_viewed(&mut self, index: usize) {
        if let Some(review) = self.reviews.get_mut(index) {
            if !review.viewed {
                review.viewed = true;
                self.save_viewed_state();
            }
        }
    }

    /// Mark all reviews as viewed
    pub fn mark_all_viewed(&mut self) {
        let mut changed = false;
        for review in &mut self.reviews {
            if !review.viewed {
                review.viewed = true;
                changed = true;
            }
        }
        if changed {
            self.save_viewed_state();
        }
    }

    /// Get the first unread review index
    pub fn first_unread_index(&self) -> Option<usize> {
        self.reviews.iter().position(|r| !r.viewed)
    }

    /// Load content for a review by index
    pub fn load_review_content(&mut self, index: usize) -> Option<&str> {
        if let Some(review) = self.reviews.get_mut(index) {
            if review.load_content().is_ok() {
                return review.content.as_deref();
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_parse_review_filename() {
        let temp_dir = TempDir::new().unwrap();
        let config = PrReviewsConfig {
            reviews_dir: temp_dir.path().to_path_buf(),
        };
        let manager = PrReviews::new(config);

        // Create a test file
        let today = Local::now().date_naive();
        let date_str = today.format("%Y-%m-%d").to_string();
        let filename = format!("PR-12345-{}.md", date_str);
        let path = temp_dir.path().join(&filename);
        let mut file = fs::File::create(&path).unwrap();
        writeln!(file, "# Test PR").unwrap();

        let review = manager.parse_review_file(&path, &date_str);
        assert!(review.is_some());
        let review = review.unwrap();
        assert_eq!(review.pr_number, 12345);
        assert_eq!(review.date, today);
    }

    #[test]
    fn test_scan_todays_reviews() {
        let temp_dir = TempDir::new().unwrap();
        let config = PrReviewsConfig {
            reviews_dir: temp_dir.path().to_path_buf(),
        };

        let today = Local::now().date_naive();
        let date_str = today.format("%Y-%m-%d").to_string();

        // Create test files
        for pr_num in [100, 200, 300] {
            let filename = format!("PR-{}-{}.md", pr_num, date_str);
            let path = temp_dir.path().join(&filename);
            fs::write(&path, format!("# PR {}", pr_num)).unwrap();
        }

        // Create an old file that shouldn't match
        let old_path = temp_dir.path().join("PR-999-2020-01-01.md");
        fs::write(&old_path, "# Old PR").unwrap();

        let manager = PrReviews::new(config);
        assert_eq!(manager.total_count(), 3);
        assert_eq!(manager.unread_count(), 3);
    }

    #[test]
    fn test_mark_viewed() {
        let temp_dir = TempDir::new().unwrap();
        let config = PrReviewsConfig {
            reviews_dir: temp_dir.path().to_path_buf(),
        };

        let today = Local::now().date_naive();
        let date_str = today.format("%Y-%m-%d").to_string();

        let filename = format!("PR-123-{}.md", date_str);
        let path = temp_dir.path().join(&filename);
        fs::write(&path, "# Test").unwrap();

        let mut manager = PrReviews::new(config);
        assert_eq!(manager.unread_count(), 1);

        manager.mark_viewed(0);
        assert_eq!(manager.unread_count(), 0);
    }
}
