//! The same application services compiled by the desktop shell, with a portable host.
//! Source paths are intentional during incremental extraction: no second business implementation.
pub mod host;
pub mod platform;
pub mod secrets;
pub mod commands {
    include!(concat!(env!("OUT_DIR"), "/command_modules.rs"));
}
#[path = "../../../apps/desktop/src-tauri/src/ai.rs"]
pub mod ai;
#[path = "../../../apps/desktop/src-tauri/src/connection.rs"]
pub mod connection;
#[path = "../../../apps/desktop/src-tauri/src/convert.rs"]
pub mod convert;
#[path = "../../../apps/desktop/src-tauri/src/dashboard.rs"]
pub mod dashboard;
#[path = "../../../apps/desktop/src-tauri/src/deploy.rs"]
pub mod deploy;
#[path = "../../../apps/desktop/src-tauri/src/dropped.rs"]
pub mod dropped;
#[path = "../../../apps/desktop/src-tauri/src/dto.rs"]
pub mod dto;
#[path = "../../../apps/desktop/src-tauri/src/error.rs"]
pub mod error;
#[path = "../../../apps/desktop/src-tauri/src/import/mod.rs"]
pub mod import;
#[path = "../../../apps/desktop/src-tauri/src/lock.rs"]
pub mod lock;
#[path = "../../../apps/desktop/src-tauri/src/mcp.rs"]
pub mod mcp;
#[path = "../../../apps/desktop/src-tauri/src/ssh.rs"]
pub mod ssh;
#[path = "../../../apps/desktop/src-tauri/src/state.rs"]
pub mod state;
#[path = "../../../apps/desktop/src-tauri/src/sync.rs"]
pub mod sync;

pub mod ssh_agent {
    // Synced agent settings remain intact; this host cannot access a desktop SSH agent.
    pub async fn resolve(_: &mut hatoba_ssh::ConnectConfig) {}
}
pub mod tray {
    pub fn refresh_close_action(_: &crate::state::AppState) {}
    pub fn set_forwards(_: &crate::host::AppHandle, _: u64, _: &[crate::dto::HostConnectionView]) {}
}

include!(concat!(env!("OUT_DIR"), "/events.rs"));
include!(concat!(env!("OUT_DIR"), "/dispatch.rs"));
include!(concat!(env!("OUT_DIR"), "/supported.rs"));

#[cfg(test)]
mod host_tests;
