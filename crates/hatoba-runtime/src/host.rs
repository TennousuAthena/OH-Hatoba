//! Host services and transport primitives. No Tauri dependency or globals.
use crate::{
    error::{AppError, AppResult},
    state::AppState,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    any::Any,
    collections::{HashMap, VecDeque},
    marker::PhantomData,
    ops::Deref,
    sync::{Arc, Mutex, OnceLock, Weak},
};

pub type Result<T> = AppResult<T>;
pub mod async_runtime {
    pub use tokio::{spawn, task::spawn_blocking};
}
pub const fn supports_local_processes() -> bool {
    false
}

#[async_trait::async_trait]
pub trait SystemServices: Send + Sync {
    /// Native host-only call. This channel is never delivered to the WebView.
    async fn call(&self, method: &str, args: Value) -> AppResult<Value>;
}
pub enum Message {
    Json(String),
    Binary(Vec<u8>),
}
#[derive(Default)]
struct Queue {
    messages: VecDeque<Message>,
    bytes: usize,
    closed: bool,
}
#[derive(Default)]
pub struct Outbox(Mutex<Queue>);
impl Outbox {
    fn push(&self, message: Message) -> AppResult<()> {
        let len = match &message {
            Message::Json(s) => s.len(),
            Message::Binary(b) => b.len(),
        };
        let mut q = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if q.closed || q.messages.len() >= 512 || q.bytes + len > 2 * 1024 * 1024 {
            return Err(AppError::internal("Native message queue unavailable"));
        }
        q.bytes += len;
        q.messages.push_back(message);
        Ok(())
    }
    pub fn pop(&self) -> Option<Message> {
        let mut q = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let message = q.messages.pop_front()?;
        q.bytes -= match &message {
            Message::Json(s) => s.len(),
            Message::Binary(b) => b.len(),
        };
        Some(message)
    }
    pub fn close(&self) {
        let mut q = self.0.lock().unwrap_or_else(|e| e.into_inner());
        q.closed = true;
        q.messages.clear();
        q.bytes = 0;
    }
    pub fn json(&self, message: Value) -> AppResult<()> {
        let encoded = serde_json::to_string(&message)
            .map_err(|_| AppError::internal("Could not encode host message"))?;
        if encoded.encode_utf16().count() > 1024 * 1024 {
            return Err(AppError::internal(
                "Native response exceeds the message limit",
            ));
        }
        self.push(Message::Json(encoded))
    }
}
pub struct PackageInfo {
    pub version: String,
}
struct Inner {
    state: OnceLock<Arc<AppState>>,
    outbox: Arc<Outbox>,
    services: Arc<dyn SystemServices>,
    channels: Mutex<HashMap<u32, Weak<ChannelInner>>>,
    package: PackageInfo,
}
#[derive(Clone)]
pub struct AppHandle(Arc<Inner>);
impl AppHandle {
    pub fn new(state: AppState, services: Arc<dyn SystemServices>) -> Self {
        let app = Self(Arc::new(Inner {
            state: OnceLock::new(),
            outbox: Arc::new(Outbox::default()),
            services,
            channels: Mutex::new(HashMap::new()),
            package: PackageInfo {
                version: env!("CARGO_PKG_VERSION").into(),
            },
        }));
        let _ = app.0.state.set(Arc::new(state));
        app
    }
    pub fn outbox(&self) -> Arc<Outbox> {
        self.0.outbox.clone()
    }
    pub fn package_info(&self) -> &PackageInfo {
        &self.0.package
    }
    pub async fn service(&self, method: &str, args: Value) -> AppResult<Value> {
        self.0.services.call(method, args).await
    }
    pub fn channel<T>(&self, id: u32) -> AppResult<ipc::Channel<T>> {
        let mut channels = self.0.channels.lock().unwrap_or_else(|e| e.into_inner());
        channels.retain(|_, c| c.strong_count() > 0);
        if channels.len() >= 128 || channels.contains_key(&id) {
            return Err(AppError::invalid(
                "channel",
                "Channel limit or duplicate channel",
            ));
        }
        let inner = Arc::new(ChannelInner {
            id,
            outbox: self.outbox(),
            credits: Arc::new(tokio::sync::Semaphore::new(512 * 1024)),
            flights: Mutex::new(VecDeque::new()),
        });
        channels.insert(id, Arc::downgrade(&inner));
        Ok(ipc::Channel {
            inner,
            marker: PhantomData,
        })
    }
    pub fn ack(&self, channel: u32, bytes: usize) -> AppResult<()> {
        let channels = self.0.channels.lock().unwrap_or_else(|e| e.into_inner());
        let Some(channel) = channels.get(&channel).and_then(Weak::upgrade) else {
            return Ok(());
        };
        let mut flights = channel.flights.lock().unwrap_or_else(|e| e.into_inner());
        let Some((expected, permit)) = flights.pop_front() else {
            return Err(AppError::invalid(
                "ack",
                "Unexpected terminal acknowledgement",
            ));
        };
        if expected != bytes {
            flights.push_front((expected, permit));
            return Err(AppError::invalid(
                "ack",
                "Invalid terminal acknowledgement size",
            ));
        }
        drop(permit);
        Ok(())
    }
}
pub struct State<'a, T>(&'a T);
impl<T> Clone for State<'_, T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}
impl<T> Deref for State<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0
    }
}
pub trait Manager {
    fn state<T: Any>(&self) -> State<'_, T>;
}
impl Manager for AppHandle {
    fn state<T: Any>(&self) -> State<'_, T> {
        let state = self.0.state.get().expect("Host state initialized").as_ref();
        State(
            (state as &dyn Any)
                .downcast_ref::<T>()
                .expect("Host state type"),
        )
    }
}
pub trait Event: Serialize {
    const NAME: &'static str;
    fn emit(&self, app: &AppHandle) -> AppResult<()> {
        app.outbox()
            .json(json!({"v":1,"type":"event","event":Self::NAME,"payload":self}))
    }
}
struct ChannelInner {
    id: u32,
    outbox: Arc<Outbox>,
    credits: Arc<tokio::sync::Semaphore>,
    flights: Mutex<VecDeque<(usize, tokio::sync::OwnedSemaphorePermit)>>,
}
impl Drop for ChannelInner {
    fn drop(&mut self) {
        let _ = self
            .outbox
            .json(json!({"v":1,"type":"channel_close","channel":self.id}));
    }
}
pub mod ipc {
    use super::*;
    pub enum InvokeResponseBody {
        Json(String),
        Raw(Vec<u8>),
    }
    pub trait IpcResponse {
        fn body(self) -> AppResult<InvokeResponseBody>;
    }
    impl<T: Serialize> IpcResponse for T {
        fn body(self) -> AppResult<InvokeResponseBody> {
            Ok(InvokeResponseBody::Json(
                serde_json::to_string(&self)
                    .map_err(|_| AppError::internal("Could not encode channel message"))?,
            ))
        }
    }
    pub struct Channel<T> {
        pub(super) inner: Arc<ChannelInner>,
        pub(super) marker: PhantomData<fn(T)>,
    }
    impl<T> Clone for Channel<T> {
        fn clone(&self) -> Self {
            Self {
                inner: self.inner.clone(),
                marker: PhantomData,
            }
        }
    }
    impl<T: IpcResponse> Channel<T> {
        pub fn send(&self, message: T) -> AppResult<()> {
            match message.body()? {
                InvokeResponseBody::Json(json) => self.inner.outbox.json(json!({"v":1,"type":"channel","channel":self.inner.id,"payload":serde_json::from_str::<Value>(&json).map_err(|_| AppError::internal("Invalid channel JSON"))?})),
                InvokeResponseBody::Raw(bytes) => {
                    let permit = self.inner.credits.clone().try_acquire_many_owned(bytes.len().try_into().map_err(|_| AppError::internal("Frame too large"))?).map_err(|_| AppError::internal("Terminal backpressure limit"))?;
                    self.raw(bytes, permit)
                }
            }
        }
        fn raw(&self, bytes: Vec<u8>, permit: tokio::sync::OwnedSemaphorePermit) -> AppResult<()> {
            if bytes.len() > 256 * 1024 {
                return Err(AppError::internal("Terminal frame too large"));
            }
            let len = bytes.len();
            let mut framed = self.inner.id.to_le_bytes().to_vec();
            framed.extend(bytes);
            let mut flights = self.inner.flights.lock().unwrap_or_else(|e| e.into_inner());
            flights.push_back((len, permit));
            if let Err(error) = self.inner.outbox.push(Message::Binary(framed)) {
                flights.pop_back();
                return Err(error);
            }
            Ok(())
        }
        pub async fn send_frame(&self, message: T) -> AppResult<()> {
            let InvokeResponseBody::Raw(bytes) = message.body()? else {
                return Err(AppError::internal("Expected terminal frame"));
            };
            if bytes.len() > 256 * 1024 {
                return Err(AppError::internal("Terminal frame too large"));
            }
            let permit = tokio::time::timeout(
                std::time::Duration::from_secs(30),
                self.inner
                    .credits
                    .clone()
                    .acquire_many_owned(bytes.len() as u32),
            )
            .await
            .map_err(|_| AppError::internal("Terminal consumer timed out"))?
            .map_err(|_| AppError::internal("Terminal consumer closed"))?;
            self.raw(bytes, permit)
        }
    }
}
pub fn argument<T: DeserializeOwned>(args: &Value, name: &str) -> AppResult<T> {
    serde_json::from_value(args.get(name).cloned().unwrap_or(Value::Null))
        .map_err(|_| AppError::invalid(name, "Invalid command argument"))
}
pub fn channel_id(args: &Value, name: &str) -> AppResult<u32> {
    args.get(name)
        .and_then(|v| v.get("channel"))
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0)
        .ok_or_else(|| AppError::invalid(name, "Invalid channel handle"))
}
pub mod clipboard {
    use super::*;
    pub async fn write_text(app: &AppHandle, text: &str) -> AppResult<()> {
        app.service("clipboard_write", json!({"text":text}))
            .await
            .map(|_| ())
    }
    pub async fn read_text(app: &AppHandle) -> AppResult<String> {
        app.service("clipboard_read", Value::Null)
            .await?
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| AppError::internal("Invalid clipboard result"))
    }
}
pub async fn open_url(app: &AppHandle, url: &str) -> AppResult<()> {
    app.service("open_url", json!({"url":url,"purpose":"authentication"}))
        .await
        .map(|_| ())
}
pub async fn transfer_active(app: &AppHandle, id: &str, active: bool) {
    let _ = app
        .service("transfer_active", json!({"id":id,"active":active}))
        .await;
}
pub async fn send_terminal<T: ipc::IpcResponse>(
    channel: &ipc::Channel<T>,
    message: T,
) -> AppResult<()> {
    channel.send_frame(message).await
}
pub async fn commit_output(app: &AppHandle, path: &std::path::Path) -> AppResult<()> {
    app.service("commit_output", json!({"path":path.to_string_lossy()}))
        .await
        .map(|_| ())
}
pub async fn input_name(app: &AppHandle, path: &std::path::Path) -> AppResult<String> {
    app.service("input_name", json!({"path":path.to_string_lossy()}))
        .await?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::invalid("path", "Invalid document name"))
}

pub async fn release_document(app: &AppHandle, path: &std::path::Path) {
    let _ = app
        .service("release_document", json!({"path":path.to_string_lossy()}))
        .await;
}
