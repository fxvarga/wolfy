//! CommandHandler - Coordinates application commands
//!
//! Handles high-level commands and coordinates use cases.

use std::sync::Arc;

use crate::application::ports::RuntimePort;
use crate::application::use_cases::{LaunchAppUseCase, SearchAppsUseCase};
use crate::domain::entities::AppItem;
use crate::domain::errors::DomainError;
use crate::domain::repositories::{AppRepository, HistoryRepository};
use crate::domain::services::search_service::SearchResult;
use crate::domain::value_objects::SearchQuery;

/// Application command
#[derive(Clone, Debug)]
pub enum AppCommand {
    /// Search for applications
    Search(String),
    /// Launch selected application
    LaunchSelected,
    /// Launch application by index
    LaunchByIndex(usize),
    /// Launch application by ID
    LaunchById(String),
    /// Move selection up
    SelectUp,
    /// Move selection down
    SelectDown,
    /// Show window
    Show,
    /// Hide window
    Hide,
    /// Toggle window visibility
    Toggle,
    /// Clear search
    Clear,
    /// Refresh application list
    Refresh,
}

/// Handler for application commands
pub struct CommandHandler<R, H, RT>
where
    R: AppRepository + 'static,
    H: HistoryRepository + 'static,
    RT: RuntimePort + 'static,
{
    search_use_case: SearchAppsUseCase<R, H>,
    launch_use_case: LaunchAppUseCase<R, H, RT>,
    current_results: Vec<SearchResult>,
    selected_index: usize,
    current_query: String,
}

impl<R, H, RT> CommandHandler<R, H, RT>
where
    R: AppRepository,
    H: HistoryRepository,
    RT: RuntimePort,
{
    /// Create a new command handler
    pub fn new(
        app_repository: Arc<R>,
        history_repository: Arc<std::sync::Mutex<H>>,
        runtime: Arc<RT>,
    ) -> Self {
        let search_use_case = SearchAppsUseCase::new(app_repository.clone(), history_repository.clone());
        let launch_use_case = LaunchAppUseCase::new(app_repository, history_repository, runtime);

        Self {
            search_use_case,
            launch_use_case,
            current_results: Vec::new(),
            selected_index: 0,
            current_query: String::new(),
        }
    }

    /// Handle a command
    pub fn handle(&mut self, command: AppCommand) -> Result<(), DomainError> {
        match command {
            AppCommand::Search(query) => {
                self.current_query = query.clone();
                self.current_results = self.search_use_case.execute(&query)?;
                self.selected_index = 0;
            }
            AppCommand::LaunchSelected => {
                if let Some(result) = self.current_results.get(self.selected_index) {
                    self.launch_use_case
                        .execute_direct(&result.item)
                        .map_err(|e| DomainError::IoError(e.to_string()))?;
                }
            }
            AppCommand::LaunchByIndex(index) => {
                if let Some(result) = self.current_results.get(index) {
                    self.launch_use_case
                        .execute_direct(&result.item)
                        .map_err(|e| DomainError::IoError(e.to_string()))?;
                }
            }
            AppCommand::LaunchById(id) => {
                self.launch_use_case
                    .execute(&id)
                    .map_err(|e| DomainError::IoError(e.to_string()))?;
            }
            AppCommand::SelectUp => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            AppCommand::SelectDown => {
                if self.selected_index < self.current_results.len().saturating_sub(1) {
                    self.selected_index += 1;
                }
            }
            AppCommand::Clear => {
                self.current_query.clear();
                self.current_results.clear();
                self.selected_index = 0;
            }
            AppCommand::Refresh => {
                self.search_use_case.refresh()?;
                // Re-execute current query
                if !self.current_query.is_empty() {
                    self.current_results = self.search_use_case.execute(&self.current_query)?;
                }
            }
            AppCommand::Show | AppCommand::Hide | AppCommand::Toggle => {
                // These are handled by WindowManager
            }
        }
        Ok(())
    }

    /// Get current search results
    pub fn results(&self) -> &[SearchResult] {
        &self.current_results
    }

    /// Get selected index
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Get selected item
    pub fn selected_item(&self) -> Option<&AppItem> {
        self.current_results.get(self.selected_index).map(|r| &r.item)
    }

    /// Get current query
    pub fn current_query(&self) -> &str {
        &self.current_query
    }

    /// Set selection index
    pub fn set_selected_index(&mut self, index: usize) {
        if index < self.current_results.len() {
            self.selected_index = index;
        }
    }
}
