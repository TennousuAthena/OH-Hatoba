//! Native app services, isolated from shared business commands.
use crate::{dto::{AppInfo, NotificationInput}, error::AppResult, host::{AppHandle, State}, state::AppState};

pub fn app_info(app: AppHandle, _state: State<'_, AppState>) -> AppInfo {
    AppInfo { version: app.package_info().version.clone(), platform: crate::platform::platform(), mica: false, local_user: String::new(), device_name: crate::platform::system::device_name(), package_manager: None }
}
pub fn activity_ping(state: State<'_, AppState>) { state.touch_activity(); }
pub async fn app_quit(app: AppHandle) -> AppResult<()> { app.service("app_quit", serde_json::Value::Null).await.map(|_| ()) }
pub async fn notification_show(app: AppHandle, notification: NotificationInput) -> AppResult<()> { app.service("notification_show", serde_json::json!({"title":notification.title,"body":notification.body,"target":notification.target})).await.map(|_| ()) }
pub async fn save_text_file(app: AppHandle, path: String, contents: String) -> AppResult<()> {
    app.service("save_text_file", serde_json::json!({"path":path,"contents":contents})).await.map(|_| ())
}
