//! Nonblocking C ABI for the shared application runtime. Rust tasks never occupy Node's
//! worker pool while waiting for a host-key prompt, network operation or AI approval.
use futures::FutureExt;
#[cfg(target_env = "ohos")]
use hatoba_core::crypto::random_key;
use hatoba_core::{Vault, crypto::Key32};
use hatoba_runtime::{
    error::{AppError, AppResult},
    host::{AppHandle, Message, Outbox, SystemServices},
    secrets::EnvelopeSecretStore,
    state::AppState,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    ffi::{CStr, CString, c_char},
    path::PathBuf,
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

static SYSTEM_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static PAGE_EPOCH: AtomicU64 = AtomicU64::new(0);
struct Services {
    queue: Arc<Outbox>,
    pending: Mutex<HashMap<u64, oneshot::Sender<AppResult<Value>>>>,
    files: Arc<crate::files::Grants>,
}
#[async_trait::async_trait]
impl SystemServices for Services {
    async fn call(&self, method: &str, args: Value) -> AppResult<Value> {
        if method == "release_document" {
            if let Some(path) = args["path"].as_str() {
                self.files.release(path);
            }
            return Ok(Value::Null);
        }
        if method == "input_name" {
            let path = args["path"]
                .as_str()
                .ok_or_else(|| AppError::invalid("path", "Invalid input document"))?;
            return self.files.input_name(path).map(Value::String);
        }
        if method == "commit_output" {
            let path = args["path"]
                .as_str()
                .ok_or_else(|| AppError::invalid("path", "Invalid export"))?;
            self.files.commit(path).await?;
            return Ok(Value::Null);
        }
        if method == "save_text_file" {
            let path = args["path"]
                .as_str()
                .ok_or_else(|| AppError::invalid("path", "Invalid export"))?;
            let contents = args["contents"]
                .as_str()
                .ok_or_else(|| AppError::invalid("contents", "Invalid export"))?;
            self.files.write_text(path, contents).await?;
            return Ok(Value::Null);
        }
        let id = SYSTEM_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let (send, receive) = oneshot::channel();
        {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            if pending.len() >= 32 {
                return Err(AppError::internal("Native system service is busy"));
            }
            pending.insert(id, send);
        }
        let queued = self
            .queue
            .json(json!({"v":1,"type":"system_call","id":id,"method":method,"args":args}));
        let result = if queued.is_ok() {
            match tokio::time::timeout(Duration::from_secs(120), receive).await {
                Ok(Ok(result)) => result,
                _ => Err(AppError::internal("Native system service did not complete")),
            }
        } else {
            Err(AppError::internal("Native system service disconnected"))
        };
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
        result
    }
}
impl Services {
    fn close(&self) {
        self.files.close();
        self.queue.close();
        self.pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
    fn complete(&self, reply: Value) -> AppResult<()> {
        let id = reply["id"]
            .as_u64()
            .ok_or_else(|| AppError::invalid("id", "Invalid system reply"))?;
        if let Some(send) = self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id)
        {
            let value = if reply["ok"] == true {
                Ok(reply["result"].clone())
            } else {
                Err(AppError::internal("Native system operation failed"))
            };
            let _ = send.send(value);
        }
        Ok(())
    }
}

struct Context {
    runtime: Option<tokio::runtime::Runtime>,
    app: AppHandle,
    services: Arc<Services>,
    pending: Arc<Mutex<HashMap<u64, CancellationToken>>>,
    last_id: AtomicU64,
    diagnostics: bool,
    network_tests: bool,
    cache: PathBuf,
}
impl Context {
    fn close(&self) {
        for token in self
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
        {
            token.cancel();
        }
        self.app.outbox().close();
        self.services.close();
        if let Some(runtime) = &self.runtime {
            runtime.block_on(async {
                let close = async {
                    hatoba_runtime::lock::lock_vault(
                        &self.app,
                        hatoba_runtime::dto::LockReason::Manual,
                    )
                    .await;
                    let sessions = hatoba_runtime::state::state(&self.app).ssh.take_all();
                    for (_, session) in sessions {
                        session.close().await;
                    }
                    let connections = hatoba_runtime::state::state(&self.app)
                        .connections
                        .take_all();
                    for connection in connections {
                        connection.close().await;
                    }
                };
                let _ = tokio::time::timeout(Duration::from_secs(3), close).await;
            });
        }
    }
    fn submit(&self, message: Value) -> AppResult<()> {
        if message["v"] != 1 {
            return Err(AppError::invalid("v", "Unsupported host protocol"));
        }
        match message["type"].as_str() {
            Some("ack") => {
                let channel = message["channel"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(|| AppError::invalid("channel", "Invalid acknowledgement"))?;
                let bytes = message["bytes"]
                    .as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .filter(|n| *n <= 256 * 1024)
                    .ok_or_else(|| AppError::invalid("bytes", "Invalid acknowledgement"))?;
                return self.app.ack(channel, bytes);
            }
            Some("cancel") => {
                if let Some(id) = message["id"].as_u64()
                    && let Some(token) = self
                        .pending
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(&id)
                {
                    token.cancel();
                }
                return Ok(());
            }
            Some("call") => {}
            _ => return Err(AppError::invalid("type", "Invalid native message")),
        }
        let id = message["id"]
            .as_u64()
            .filter(|n| *n > 0 && *n < 9_007_199_254_740_991)
            .ok_or_else(|| AppError::invalid("id", "Invalid request id"))?;
        if id <= self.last_id.fetch_max(id, Ordering::Relaxed) {
            return Err(AppError::invalid("id", "Request id must increase"));
        }
        let command = message["command"]
            .as_str()
            .filter(|s| s.len() <= 128)
            .ok_or_else(|| AppError::invalid("command", "Invalid command"))?
            .to_owned();
        let args = message["args"]
            .as_object()
            .map(|m| Value::Object(m.clone()))
            .ok_or_else(|| AppError::invalid("args", "Invalid command arguments"))?;
        let token = CancellationToken::new();
        {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            if pending.len() >= 64 {
                return Err(AppError::internal("Native request queue is full"));
            }
            pending.insert(id, token.clone());
        }
        let app = self.app.clone();
        let pending = self.pending.clone();
        let files = self.services.files.clone();
        let network_tests = self.diagnostics && self.network_tests;
        let cache = self.cache.clone();
        self.runtime.as_ref().ok_or_else(|| AppError::internal("Native runtime stopped"))?.spawn(async move {
            let operation = async {
                if command == "host_probe" {
                    let report = tokio::task::spawn_blocking(crate::probe).await.map_err(|_| AppError::internal("Probe failed"))?;
                    return serde_json::to_value(report).map_err(|_| AppError::internal("Probe encoding failed"));
                }
                if command == "host_network_probe" {
                    if !network_tests { return Ok(json!({"enabled":false})); }
                    return Ok(crate::network_probe::run(&cache).await);
                }
                if command == "host_service" {
                    let method = args["method"].as_str().ok_or_else(|| AppError::invalid("method", "Missing host service"))?;
                    if !["capabilities", "clipboard_write", "clipboard_read", "open_url", "pick_open_path", "pick_open_files", "pick_save_path", "window_focused", "notification_permission", "notifications_ready"].contains(&method) {
                        return Err(AppError::invalid("method", "Unavailable host service"));
                    }
                    return app.service(method, args.clone()).await;
                }
                let mut args = args;
                let commit = files.prepare(&command, &mut args)?;
                let preview_args = (command == "hosts_import_preview").then(|| args.clone());
                let release_after = match command.as_str() {
                    "key_import" => args.pointer("/input/path"),
                    "skill_import" | "hosts_import" | "vault_export_backup" | "skill_export" | "save_text_file" => args.get("path"),
                    _ => None,
                }.and_then(Value::as_str).map(str::to_owned);
                let mut result = hatoba_runtime::dispatch(&app, &command, args).await?;
                if let Some(args) = preview_args { files.review_import_preview(&args, &mut result)?; }
                if let Some(path) = commit { files.commit(&path).await?; }
                files.public_value(&mut result);
                if let Some(path) = release_after { files.release(&path); }
                Ok(result)
            };
            let result = tokio::select! {
                _ = token.cancelled() => None,
                result = std::panic::AssertUnwindSafe(operation).catch_unwind() => Some(result.unwrap_or_else(|_| Err(AppError::internal("Native command failed")))),
            };
            pending.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            if let Some(result) = result {
                let reply = match result { Ok(value) => json!({"v":1,"type":"reply","id":id,"ok":true,"result":value}), Err(error) => json!({"v":1,"type":"reply","id":id,"ok":false,"error":error}) };
                if app.outbox().json(reply).is_err() {
                    let _ = app.outbox().json(json!({"v":1,"type":"reply","id":id,"ok":false,"error":AppError::internal("Native response exceeded the message limit or the queue is busy")}));
                }
            }
        });
        Ok(())
    }
}
impl Drop for Context {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
#[derive(Default)]
struct Holder {
    context: Option<Arc<Context>>,
    closing: bool,
}
fn holder() -> &'static (Mutex<Holder>, Condvar) {
    static HOLDER: OnceLock<(Mutex<Holder>, Condvar)> = OnceLock::new();
    HOLDER.get_or_init(|| (Mutex::new(Holder::default()), Condvar::new()))
}
fn context() -> AppResult<Arc<Context>> {
    holder()
        .0
        .try_lock()
        .map_err(|_| AppError::internal("Native host is busy"))?
        .context
        .clone()
        .ok_or_else(|| AppError::internal("Native host is not initialized"))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Config {
    files_dir: PathBuf,
    cache_dir: PathBuf,
    device_name: String,
    locale: String,
    system_ca_path: Option<PathBuf>,
    #[serde(default)]
    biometric_available: bool,
    #[serde(default)]
    diagnostics: bool,
    #[serde(default)]
    network_tests: bool,
}
fn initialize(config: Config, epoch: u64) -> AppResult<()> {
    let (mutex, changed) = holder();
    let mut holder = mutex.lock().unwrap_or_else(|e| e.into_inner());
    while holder.closing {
        holder = changed.wait(holder).unwrap_or_else(|e| e.into_inner());
    }
    if epoch != PAGE_EPOCH.load(Ordering::Acquire) {
        return Err(AppError::internal(
            "Native page changed during initialization",
        ));
    }
    if holder.context.is_some() {
        return Ok(());
    }
    if !config.files_dir.is_absolute() {
        return Err(AppError::invalid(
            "filesDir",
            "Native sandbox path must be absolute",
        ));
    }
    hatoba_runtime::platform::system::configure(&config.device_name, &config.locale);
    hatoba_runtime::platform::biometric::configure(config.biometric_available);
    #[cfg(target_env = "ohos")]
    {
        let path = config
            .system_ca_path
            .as_deref()
            .ok_or_else(|| AppError::internal("System certificate path is required"))?;
        hatoba_net::configure_system_ca(path).map_err(AppError::internal)?;
    }
    #[cfg(not(target_env = "ohos"))]
    let _ = &config.system_ca_path;
    let directory = config.files_dir.join("hatoba");
    std::fs::create_dir_all(&directory)?;
    let secrets = Arc::new(EnvelopeSecretStore::new(
        &directory.join("secrets"),
        device_key()?,
    )?);
    let vault = Vault::open(&directory.join("vault.sqlite"))?;
    crate::files::Grants::cleanup_stale(&config.cache_dir)?;
    let services = Arc::new(Services {
        queue: Arc::new(Outbox::default()),
        pending: Mutex::new(HashMap::new()),
        files: Arc::new(crate::files::Grants::new(&config.cache_dir)?),
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()?;
    let app = {
        let _entered = runtime.enter();
        AppHandle::new(
            AppState::with_secrets(vault, false, secrets),
            services.clone(),
        )
    };
    if epoch != PAGE_EPOCH.load(Ordering::Acquire) {
        return Err(AppError::internal(
            "Native page changed during initialization",
        ));
    }
    runtime.block_on(async {
        hatoba_runtime::lock::spawn_watchers(app.clone());
        hatoba_runtime::sync::spawn_scheduler(app.clone());
    });
    holder.context = Some(Arc::new(Context {
        runtime: Some(runtime),
        app,
        services,
        pending: Arc::new(Mutex::new(HashMap::new())),
        last_id: AtomicU64::new(0),
        diagnostics: config.diagnostics,
        network_tests: config.network_tests,
        cache: config.cache_dir,
    }));
    Ok(())
}
fn device_key() -> AppResult<Key32> {
    #[cfg(target_env = "ohos")]
    {
        unsafe extern "C" {
            fn hatoba_device_key(out: *mut u8, candidate: *const u8) -> i32;
        }
        let candidate = random_key()?;
        let mut key = random_key()?;
        let status = unsafe { hatoba_device_key(key.as_mut_ptr(), candidate.as_ptr()) };
        if status != 0 {
            return Err(AppError::internal(
                "Native secure key storage is unavailable",
            ));
        }
        Ok(key)
    }
    #[cfg(not(target_env = "ohos"))]
    {
        Err(AppError::internal("HarmonyOS Asset Store is required"))
    }
}

unsafe fn input<'a>(value: *const c_char) -> AppResult<&'a str> {
    if value.is_null() {
        return Err(AppError::invalid("message", "Missing native message"));
    }
    let text = unsafe { CStr::from_ptr(value) }
        .to_str()
        .map_err(|_| AppError::invalid("message", "Invalid UTF-8"))?;
    if text.len() > 4 * 1024 * 1024 {
        return Err(AppError::invalid("message", "Native message is too large"));
    }
    Ok(text)
}
fn encoded(result: AppResult<()>) -> *mut c_char {
    let value = match result {
        Ok(()) => json!({"ok":true}),
        Err(error) => json!({"ok":false,"error":error}),
    };
    CString::new(value.to_string())
        .expect("JSON has no literal NUL")
        .into_raw()
}
#[unsafe(no_mangle)]
pub extern "C" fn hatoba_page_epoch() -> u64 {
    PAGE_EPOCH.load(Ordering::Acquire)
}

/// # Safety
/// The pointer must identify a valid, NUL-terminated UTF-8 configuration string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hatoba_initialize(config: *const c_char, epoch: u64) -> *mut c_char {
    let result = std::panic::catch_unwind(|| -> AppResult<()> {
        let text = unsafe { input(config)? };
        initialize(
            serde_json::from_str(text)
                .map_err(|_| AppError::invalid("config", "Invalid native host configuration"))?,
            epoch,
        )
    });
    encoded(result.unwrap_or_else(|_| Err(AppError::internal("Native host initialization failed"))))
}
/// # Safety
/// The pointer must identify a valid, NUL-terminated message string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hatoba_submit(message: *const c_char) -> *mut c_char {
    encoded((|| {
        let text = unsafe { input(message)? };
        let message = serde_json::from_str(text)
            .map_err(|_| AppError::invalid("message", "Invalid native JSON"))?;
        context()?.submit(message)
    })())
}
/// # Safety
/// The pointer must identify a valid, NUL-terminated host-only system reply string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hatoba_system_reply(message: *const c_char) -> *mut c_char {
    encoded((|| {
        let text = unsafe { input(message)? };
        let reply = serde_json::from_str(text)
            .map_err(|_| AppError::invalid("message", "Invalid system JSON"))?;
        context()?.services.complete(reply)
    })())
}

/// # Safety
/// Native UI lifecycle event as a NUL-terminated UTF-8 string; not callable through Web IPC.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hatoba_lifecycle(event: *const c_char) -> *mut c_char {
    encoded((|| {
        let event = unsafe { input(event)? };
        let context = context()?;
        let app = context.app.clone();
        let idle = hatoba_runtime::state::state(&app).idle_for();
        let runtime = context
            .runtime
            .as_ref()
            .ok_or_else(|| AppError::internal("Native runtime stopped"))?;
        match event {
            "foreground" => {
                runtime.spawn(async move {
                    hatoba_runtime::lock::on_resume(&app, idle).await;
                });
            }
            "screen_locked" => {
                runtime.spawn(async move {
                    hatoba_runtime::lock::on_system_sleep(&app).await;
                });
            }
            _ => {
                use hatoba_runtime::host::Event;
                let target: hatoba_runtime::dto::NotificationTarget =
                    serde_json::from_str(event)
                        .map_err(|_| AppError::invalid("event", "Unknown lifecycle event"))?;
                if [&target.tab, &target.conversation_id, &target.host_id]
                    .into_iter()
                    .flatten()
                    .any(|s| s.len() > 128)
                {
                    return Err(AppError::invalid("target", "Invalid notification target"));
                }
                let state = hatoba_runtime::state::state(&app);
                let vault = state
                    .vault
                    .try_lock()
                    .map_err(|_| AppError::internal("Vault is busy"))?;
                if !vault.is_unlocked() {
                    return Err(AppError::internal("Vault is locked"));
                }
                target.emit(&app)?;
            }
        }
        Ok(())
    })())
}

/// # Safety
/// The descriptor must remain open through this call and come from the native document picker.
/// The name must be a valid NUL-terminated UTF-8 filename string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hatoba_grant(fd: i32, name: *const c_char, write: bool) -> *mut c_char {
    let result = (|| {
        let name = unsafe { input(name)? };
        unsafe { context()?.services.files.register(fd, name, write) }
    })();
    let value = match result {
        Ok(token) => json!({"ok":true,"result":token}),
        Err(error) => json!({"ok":false,"error":error}),
    };
    CString::new(value.to_string())
        .expect("JSON has no literal NUL")
        .into_raw()
}

#[repr(C)]
pub struct NativePacket {
    pub kind: u32,
    pub data: *mut u8,
    pub size: usize,
}
#[unsafe(no_mangle)]
pub extern "C" fn hatoba_poll(system: bool) -> *mut NativePacket {
    let Ok(context) = context() else {
        return std::ptr::null_mut();
    };
    let message = if system {
        context.services.queue.pop()
    } else {
        context.app.outbox().pop()
    };
    let Some(message) = message else {
        return std::ptr::null_mut();
    };
    let (kind, bytes) = match message {
        Message::Json(text) => (1, text.into_bytes()),
        Message::Binary(bytes) => (2, bytes),
    };
    let mut bytes = bytes.into_boxed_slice();
    let packet = NativePacket {
        kind,
        data: bytes.as_mut_ptr(),
        size: bytes.len(),
    };
    std::mem::forget(bytes);
    Box::into_raw(Box::new(packet))
}
/// # Safety
/// The pointer must be null or a packet returned by hatoba_poll, not already freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hatoba_packet_free(packet: *mut NativePacket) {
    if packet.is_null() {
        return;
    }
    let packet = unsafe { Box::from_raw(packet) };
    let mut bytes =
        unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(packet.data, packet.size)) };
    use zeroize::Zeroize;
    bytes.zeroize();
}
#[unsafe(no_mangle)]
pub extern "C" fn hatoba_detach() {
    PAGE_EPOCH.fetch_add(1, Ordering::AcqRel);
    // Initialization runs on a Node worker; do not block the UI on an Asset Store call.
    let (mutex, _) = holder();
    let Ok(mut held) = mutex.try_lock() else {
        return;
    };
    let Some(context) = held.context.take() else {
        return;
    };
    held.closing = true;
    context.app.outbox().close();
    context.services.close();
    std::thread::spawn(move || {
        context.close();
        drop(context);
        let (mutex, changed) = holder();
        mutex.lock().unwrap_or_else(|e| e.into_inner()).closing = false;
        changed.notify_all();
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn initialization_queued_before_detach_cannot_attach_to_the_new_page() {
        let old_epoch = super::hatoba_page_epoch();
        super::hatoba_detach();
        let config = super::Config {
            files_dir: std::env::temp_dir(),
            cache_dir: std::env::temp_dir(),
            device_name: "generated".into(),
            locale: "en".into(),
            system_ca_path: None,
            biometric_available: false,
            diagnostics: true,
            network_tests: false,
        };
        let error = super::initialize(config, old_epoch).unwrap_err();
        assert!(error.detail.contains("page changed"));
    }
}
