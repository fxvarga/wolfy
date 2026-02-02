//! Gateways - Repository implementations

pub mod file_history_gateway;
pub mod memory_app_gateway;

pub use file_history_gateway::FileHistoryGateway;
pub use memory_app_gateway::MemoryAppGateway;
