//! Desktop implementation of the application host boundary.
pub use tauri::{AppHandle, Manager, Result, State, async_runtime, ipc};
pub use tauri_specta::Event;
pub const fn supports_local_processes() -> bool {
    true
}

pub mod clipboard {
    use super::AppHandle;
    use crate::error::{AppError, AppResult};
    use tauri_plugin_clipboard_manager::ClipboardExt;
    pub async fn write_text(app: &AppHandle, text: &str) -> AppResult<()> {
        app.clipboard()
            .write_text(text)
            .map_err(|e| AppError::internal(e.to_string()))
    }
    pub async fn read_text(app: &AppHandle) -> AppResult<String> {
        app.clipboard()
            .read_text()
            .map_err(|e| AppError::internal(e.to_string()))
    }
}
pub async fn open_url(app: &AppHandle, url: &str) -> crate::error::AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| crate::error::AppError::internal(e.to_string()))
}
pub async fn send_terminal<T: ipc::IpcResponse>(
    channel: &ipc::Channel<T>,
    message: T,
) -> crate::error::AppResult<()> {
    channel
        .send(message)
        .map_err(|e| crate::error::AppError::internal(e.to_string()))
}
pub async fn commit_output(_: &AppHandle, _: &std::path::Path) -> crate::error::AppResult<()> {
    Ok(())
}
pub async fn transfer_active(_: &AppHandle, _: &str, _: bool) {}
pub async fn input_name(_: &AppHandle, path: &std::path::Path) -> crate::error::AppResult<String> {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| crate::error::AppError::invalid("path", "not a file"))
}

pub async fn release_document(_: &AppHandle, _: &std::path::Path) {}
