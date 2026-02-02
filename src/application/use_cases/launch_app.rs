//! LaunchAppUseCase - Launch an application
//!
//! Handles the business logic for launching an application.

use std::sync::Arc;

use crate::application::ports::RuntimePort;
use crate::domain::entities::AppItem;
use crate::domain::errors::DomainError;
use crate::domain::repositories::{AppRepository, HistoryRepository};

/// Error for launch operations
#[derive(Debug)]
pub enum LaunchError {
    /// Application not found
    NotFound(String),
    /// Failed to launch
    LaunchFailed(String),
    /// Domain error
    Domain(DomainError),
}

impl std::fmt::Display for LaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LaunchError::NotFound(id) => write!(f, "Application not found: {}", id),
            LaunchError::LaunchFailed(msg) => write!(f, "Launch failed: {}", msg),
            LaunchError::Domain(e) => write!(f, "Domain error: {}", e),
        }
    }
}

impl std::error::Error for LaunchError {}

impl From<DomainError> for LaunchError {
    fn from(e: DomainError) -> Self {
        LaunchError::Domain(e)
    }
}

/// Use case for launching applications
pub struct LaunchAppUseCase<R, H, RT>
where
    R: AppRepository,
    H: HistoryRepository,
    RT: RuntimePort,
{
    app_repository: Arc<R>,
    history_repository: Arc<std::sync::Mutex<H>>,
    runtime: Arc<RT>,
}

impl<R, H, RT> LaunchAppUseCase<R, H, RT>
where
    R: AppRepository,
    H: HistoryRepository,
    RT: RuntimePort,
{
    /// Create a new launch app use case
    pub fn new(
        app_repository: Arc<R>,
        history_repository: Arc<std::sync::Mutex<H>>,
        runtime: Arc<RT>,
    ) -> Self {
        Self {
            app_repository,
            history_repository,
            runtime,
        }
    }

    /// Launch an application by ID
    pub fn execute(&self, app_id: &str) -> Result<(), LaunchError> {
        // 1. Find the application
        let app = self
            .app_repository
            .find_by_id(app_id)?
            .ok_or_else(|| LaunchError::NotFound(app_id.to_string()))?;

        // 2. Launch it
        self.launch_app(&app)?;

        // 3. Record in history
        if let Ok(mut history) = self.history_repository.lock() {
            let _ = history.record_launch(app_id);
        }

        Ok(())
    }

    /// Launch an application directly (without lookup)
    pub fn execute_direct(&self, app: &AppItem) -> Result<(), LaunchError> {
        // 1. Launch it
        self.launch_app(app)?;

        // 2. Record in history
        if let Ok(mut history) = self.history_repository.lock() {
            let _ = history.record_launch(&app.id);
        }

        Ok(())
    }

    /// Internal launch logic
    fn launch_app(&self, app: &AppItem) -> Result<(), LaunchError> {
        self.runtime
            .execute(&app.path)
            .map_err(|e| LaunchError::LaunchFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::repositories::app_repository::NullAppRepository;
    use crate::domain::repositories::history_repository::NullHistoryRepository;
    use crate::application::ports::runtime_port::NullRuntimePort;
    use std::sync::Mutex;

    #[test]
    fn test_launch_direct() {
        let app_repo = Arc::new(NullAppRepository);
        let history_repo = Arc::new(Mutex::new(NullHistoryRepository));
        let runtime = Arc::new(NullRuntimePort);

        let use_case = LaunchAppUseCase::new(app_repo, history_repo, runtime);

        let app = AppItem::new("Test", "/test/app.exe");
        let result = use_case.execute_direct(&app);

        assert!(result.is_ok());
    }
}
