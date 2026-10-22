use crate::{
    dispatch,
    host::{
        self, AppHandle, Message, SystemServices,
        ipc::{InvokeResponseBody, IpcResponse},
    },
    state::AppState,
};
use hatoba_core::{KdfParams, Vault, platform::MemorySecretStore};
use serde_json::{Value, json};
use std::sync::Arc;

struct Services;
#[async_trait::async_trait]
impl SystemServices for Services {
    async fn call(&self, _: &str, _: Value) -> crate::error::AppResult<Value> {
        Err(crate::error::AppError::internal(
            "No OS services in this test",
        ))
    }
}
fn app() -> AppHandle {
    let mut vault = Vault::open_in_memory().unwrap();
    vault
        .create_with_params("generated portable test only", KdfParams::for_tests())
        .unwrap();
    AppHandle::new(
        AppState::with_secrets(vault, false, Arc::new(MemorySecretStore::new())),
        Arc::new(Services),
    )
}
#[tokio::test]
async fn generated_dispatch_uses_real_vault_commands_and_preserves_agent_configuration() {
    let app = app();
    assert!(crate::SUPPORTED_COMMANDS.len() > 170);
    assert!(!crate::SUPPORTED_COMMANDS.contains(&"local_open"));
    assert_eq!(
        dispatch(&app, "app_info", json!({})).await.unwrap()["platform"],
        "harmonyos"
    );
    assert_eq!(
        dispatch(&app, "vault_status", json!({})).await.unwrap()["state"],
        "unlocked"
    );
    let saved = dispatch(&app, "host_save", json!({"input":{
        "name":"generated fixture", "address":"127.0.0.1", "port":22, "username":"fixture", "auth_kind":"agent",
        "tags":["device-test"], "favorite":false, "proxy_mode":"direct", "note":"", "ai_notes":"", "env":[]
    }})).await.unwrap();
    assert_eq!(saved["auth_kind"], "agent");
    let listed = dispatch(&app, "hosts_list", json!({})).await.unwrap();
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["auth_kind"], "agent");
    assert!(
        dispatch(&app, "host_save", json!({"input":{}}))
            .await
            .is_err()
    );
    assert!(dispatch(&app, "local_open", json!({})).await.is_err());
    dispatch(&app, "vault_lock", json!({})).await.unwrap();
    assert!(dispatch(&app, "hosts_list", json!({})).await.is_err());
}

struct Frame(Vec<u8>);
impl IpcResponse for Frame {
    fn body(self) -> host::Result<InvokeResponseBody> {
        Ok(InvokeResponseBody::Raw(self.0))
    }
}
#[tokio::test]
async fn terminal_credits_require_exact_ack_and_channel_close_follows_frames() {
    let app = app();
    let channel = app.channel::<Frame>(17).unwrap();
    assert!(app.channel::<Frame>(17).is_err());
    channel
        .send_frame(Frame(vec![0xff; 256 * 1024]))
        .await
        .unwrap();
    channel
        .send_frame(Frame(vec![0; 256 * 1024]))
        .await
        .unwrap();
    {
        let blocked = channel.send_frame(Frame(vec![1; 1]));
        tokio::pin!(blocked);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), &mut blocked)
                .await
                .is_err()
        );
        assert!(app.ack(17, 1).is_err()); // A forged credit cannot release the producer.
        app.ack(17, 256 * 1024).unwrap();
        blocked.await.unwrap();
    }
    let Message::Binary(frame) = app.outbox().pop().unwrap() else {
        panic!("binary frame")
    };
    assert_eq!(&frame[..4], &17u32.to_le_bytes());
    assert_eq!(&frame[4..], vec![0xff; 256 * 1024]);
    app.ack(17, 256 * 1024).unwrap();
    app.ack(17, 1).unwrap();
    assert!(app.ack(17, 1).is_err());
    app.outbox().pop();
    app.outbox().pop();
    drop(channel);
    let Message::Json(close) = app.outbox().pop().unwrap() else {
        panic!("close message")
    };
    assert_eq!(
        serde_json::from_str::<Value>(&close).unwrap()["type"],
        "channel_close"
    );
}
#[tokio::test]
async fn queues_are_bounded_and_closed_queues_fail_visibly() {
    let app = app();
    let out = app.outbox();
    assert!(
        out.json(json!({"generated":"x".repeat(1024 * 1024 + 1)}))
            .is_err()
    );
    for _ in 0..512 {
        out.json(Value::Null).unwrap();
    }
    assert!(out.json(Value::Null).is_err());
    out.close();
    assert!(out.pop().is_none());
    assert!(out.json(Value::Null).is_err());
    assert!(
        app.channel::<Frame>(1)
            .unwrap()
            .send(Frame(vec![0; 1]))
            .is_err()
    );
}

#[tokio::test]
async fn synced_stdio_configuration_is_preserved_but_never_started() {
    let app = app();
    assert!(!host::supports_local_processes());
    let saved = dispatch(&app, "mcp_server_save", json!({"input":{
        "name":"generated-stdio-fixture", "transport":{"kind":"stdio", "command":"a-command-that-must-never-run", "args":["fixture"], "env":[{"key":"GENERATED_VALUE", "value":"generated-test-only"}]}, "always_ask":true
    }})).await.unwrap();
    let id = saved["id"].as_str().unwrap();
    assert_eq!(saved["enabled"], false);
    dispatch(
        &app,
        "mcp_server_set_enabled",
        json!({"id":id, "enabled":true}),
    )
    .await
    .unwrap();
    let status = dispatch(&app, "mcp_server_start", json!({"id":id}))
        .await
        .unwrap();
    assert_eq!(status["state"], "failed");
    let servers = dispatch(&app, "mcp_servers_list", json!({})).await.unwrap();
    assert_eq!(servers[0]["enabled"], false);
    assert_eq!(
        servers[0]["transport"]["command"],
        "a-command-that-must-never-run"
    );
    assert_eq!(
        servers[0]["transport"]["env_keys"],
        json!(["GENERATED_VALUE"])
    );
    assert!(!servers.to_string().contains("generated-test-only"));
    let vault = crate::state::state(&app);
    let vault = vault.vault();
    let (_, stored) = vault
        .mcp_servers()
        .into_iter()
        .find(|(key, _)| key == id)
        .unwrap();
    let hatoba_core::model::McpTransport::Stdio { env, .. } = stored.transport else {
        panic!("preserve transport");
    };
    assert_eq!(env["GENERATED_VALUE"].as_str(), "generated-test-only");
}
