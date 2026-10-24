//! Platform entry points used by the shared application services.
pub fn platform_name() -> &'static str {
    "harmonyos"
}
pub fn platform() -> crate::dto::Platform {
    crate::dto::Platform::Harmonyos
}
pub fn os_label() -> String {
    "HarmonyOS".into()
}
pub mod system {
    use std::sync::{OnceLock, RwLock};
    static METADATA: OnceLock<RwLock<(String, String)>> = OnceLock::new();
    fn metadata() -> &'static RwLock<(String, String)> {
        METADATA.get_or_init(|| RwLock::new(("HarmonyOS device".into(), "en".into())))
    }
    pub fn configure(name: &str, language: &str) {
        *metadata().write().unwrap_or_else(|e| e.into_inner()) = (
            name.chars().take(128).collect(),
            language.chars().take(32).collect(),
        );
    }
    pub fn default_user() -> String {
        String::new()
    }
    pub fn device_name() -> String {
        metadata()
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .0
            .clone()
    }
    pub fn locale() -> String {
        metadata()
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .1
            .clone()
    }
}
pub mod idle {
    pub fn system_idle() -> Option<std::time::Duration> {
        None
    }
}
pub mod power {
    pub fn watch_sleep() -> tokio::sync::mpsc::Receiver<()> {
        let (_, rx) = tokio::sync::mpsc::channel(1);
        rx
    }
}
pub mod login_shell {
    pub fn default_path() -> Option<&'static std::ffi::OsStr> {
        None
    }
}
pub mod window {
    /// The interface zoom levels in percent, kept in step with the desktop host.
    pub const ZOOM_STEPS: [u32; 10] = [67, 75, 80, 90, 100, 110, 125, 150, 175, 200];

    /// The zoom step nearest to `percent`.
    pub fn zoom_step(percent: u32) -> u32 {
        ZOOM_STEPS
            .into_iter()
            .min_by_key(|step| step.abs_diff(percent))
            .unwrap_or(100)
    }
}
pub mod biometric;
