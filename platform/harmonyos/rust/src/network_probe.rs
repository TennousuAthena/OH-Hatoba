//! Explicit diagnostics only, fixed loopback fixture endpoints, generated data, boolean results.
use hatoba_ssh::{
    AuthMethod, ConnectConfig, DynamicForward, HostKeyInfo, HostKeyVerifier, LocalForward,
    RemoteForward, ShellEvent, ShellOptions, SshErrorKind,
};
use serde_json::{Value, json};
use std::{os::fd::AsRawFd, path::Path, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_util::sync::CancellationToken;

struct Verifier(bool);
#[async_trait::async_trait]
impl HostKeyVerifier for Verifier {
    async fn verify(&self, _: &str, _: u16, _: &HostKeyInfo) -> bool {
        self.0
    }
}
fn config() -> ConnectConfig {
    let mut config = ConnectConfig::new(
        "127.0.0.1",
        22222,
        "fixture",
        AuthMethod::Password("generated-diagnostic-only".to_string().into()),
    );
    config.connect_timeout = Duration::from_secs(5);
    config
}

pub async fn run(cache: &Path) -> Value {
    let mut checks = serde_json::Map::new();
    checks.insert(
        "documentConfigSkillZipFolderAndGrantRevocation".into(),
        json!(crate::document_probe::run(cache).await.unwrap_or(false)),
    );
    let http = hatoba_ai::provider::http_client();
    let mut trusted_https = false;
    for url in ["https://www.huawei.com/", "https://example.com/"] {
        if http
            .get(url)
            .timeout(Duration::from_secs(8))
            .send()
            .await
            .is_ok_and(|response| response.url().scheme() == "https")
        {
            trusted_https = true;
            break;
        }
    }
    // Any HTTPS response proves the TLS handshake, including an HTTP 403 from the website.
    checks.insert("systemCaHttps".into(), json!(trusted_https));
    let rejected = http
        .get("https://127.0.0.1:28443/")
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .err()
        .is_some_and(|error| {
            use std::error::Error;
            let mut current: Option<&(dyn Error + 'static)> = Some(&error);
            while let Some(cause) = current {
                if cause
                    .to_string()
                    .to_ascii_lowercase()
                    .contains("certificate")
                {
                    return true;
                }
                current = cause.source();
            }
            false // A refused connection is not evidence that certificate verification worked.
        });
    checks.insert("untrustedCertificateRejected".into(), json!(rejected));
    checks.insert(
        "sshHostKeyRejection".into(),
        json!(
            hatoba_ssh::connect(config(), Arc::new(Verifier(false)))
                .await
                .err()
                .is_some_and(|e| e.kind == SshErrorKind::HostKeyRejected)
        ),
    );
    let session = match hatoba_ssh::connect(config(), Arc::new(Verifier(true))).await {
        Ok(session) => {
            checks.insert("sshHandshakeAndAuthentication".into(), json!(true));
            session
        }
        Err(_) => {
            checks.insert("sshHandshakeAndAuthentication".into(), json!(false));
            return json!({"checks":checks});
        }
    };
    checks.insert(
        "sshExec".into(),
        json!(
            session
                .exec("ohhatoba-diagnostic", None)
                .await
                .is_ok_and(|(status, bytes)| status == 0 && bytes == b"fixture-exec-ok\n")
        ),
    );
    let shell = async {
        let (shell, mut events) = session.open_shell(ShellOptions::default()).await?;
        let expected = b"generated \xff\x00 \xe4\xb8\xad\xe6\x96\x87 \xf0\x9f\x8c\x8a\r\n";
        shell.write(expected.to_vec()).await?;
        let mut actual = Vec::new();
        while actual.len() < expected.len() {
            match events.recv().await {
                Some(ShellEvent::Data(bytes)) => actual.extend(bytes),
                _ => break,
            }
        }
        shell.resize(132, 40).await;
        shell.close().await;
        Ok::<_, hatoba_ssh::SshError>(actual == expected)
    };
    checks.insert(
        "ptyBinaryUnicodeAndResize".into(),
        json!(
            tokio::time::timeout(Duration::from_secs(8), shell)
                .await
                .is_ok_and(|r| r.unwrap_or(false))
        ),
    );
    let mut file_stage = "create";
    let file_test = async {
        let area = cache.join(format!("hatoba-network-probe-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&area)?;
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(area.clone());
        let bytes: Vec<u8> = (0..524_288).map(|n| (n % 251) as u8).collect();
        file_stage = "write_fixture";
        let input = area.join("input.bin");
        std::fs::write(&input, &bytes)?;
        file_stage = "grant_directory";
        let grant = crate::files::Grants::new(&area)?;
        file_stage = "open_fixture";
        let file = std::fs::File::open(&input)?;
        file_stage = "register_read_descriptor";
        let read = unsafe { grant.register(file.as_raw_fd(), "upload.bin", false) }?;
        drop(file);
        file_stage = "sftp";
        let sftp = session.sftp().await?;
        file_stage = "list";
        let listing = sftp.list("/").await?;
        if !listing.iter().any(|f| f.name == "fixture.bin") {
            return Ok::<_, hatoba_runtime::error::AppError>(false);
        }
        file_stage = "upload";
        sftp.upload(
            Path::new(&grant.resolve(&read, false)?),
            "/upload.bin",
            |_| {},
            CancellationToken::new(),
        )
        .await?;
        let output = area.join("selected.bin");
        let file = std::fs::File::create(&output)?;
        let write = unsafe { grant.register(file.as_raw_fd(), "selected.bin", true) }?;
        drop(file);
        let staged = grant.resolve(&write, true)?;
        file_stage = "download";
        sftp.download(
            "/upload.bin",
            Path::new(&staged),
            |_| {},
            CancellationToken::new(),
        )
        .await?;
        file_stage = "commit";
        grant.commit(&staged).await?;
        file_stage = "compare";
        let valid = std::fs::read(output)? == bytes;
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let aborted = sftp
            .download(
                "/upload.bin",
                &area.join("cancelled.bin"),
                |_| {},
                cancelled,
            )
            .await
            .err()
            .is_some_and(|e| e.kind == SshErrorKind::Cancelled);
        file_stage = "cleanup";
        sftp.remove_file("/upload.bin").await?;
        Ok(valid && aborted && !area.join("cancelled.bin").exists())
    };
    let file_result = tokio::time::timeout(Duration::from_secs(15), file_test).await;
    let file_errno = match &file_result {
        Ok(Err(error)) => error
            .detail
            .rsplit("(os error ")
            .next()
            .and_then(|s| s.strip_suffix(')'))
            .and_then(|s| s.parse::<i32>().ok()),
        _ => None,
    };
    #[cfg(feature = "test-tools")]
    if let Ok(Err(error)) = &file_result {
        eprintln!("Generated SFTP fixture failed: {}", error.detail);
    }
    checks.insert(
        "sftpUploadDownloadDocumentGrantsAndCancel".into(),
        json!(file_result.is_ok_and(|r| r.unwrap_or(false))),
    );
    let file_stage = file_stage.to_owned();
    let local = async {
        let forward = session
            .local_forward(LocalForward {
                bind_address: "127.0.0.1".into(),
                bind_port: 0,
                dest_host: "fixture.invalid".into(),
                dest_port: 7,
            })
            .await?;
        let mut stream = TcpStream::connect(("127.0.0.1", forward.port())).await?;
        stream.write_all(b"forward-fixture").await?;
        let mut bytes = [0; 15];
        stream.read_exact(&mut bytes).await?;
        forward.stop();
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(&bytes == b"forward-fixture")
    };
    checks.insert(
        "localForward".into(),
        json!(
            tokio::time::timeout(Duration::from_secs(5), local)
                .await
                .is_ok_and(|r| r.unwrap_or(false))
        ),
    );
    let socks = async {
        let forward = session
            .dynamic_forward(DynamicForward {
                bind_address: "127.0.0.1".into(),
                bind_port: 0,
            })
            .await?;
        let mut stream = TcpStream::connect(("127.0.0.1", forward.port())).await?;
        stream.write_all(&[5, 1, 0]).await?;
        let mut greeting = [0; 2];
        stream.read_exact(&mut greeting).await?;
        let mut request = vec![5, 1, 0, 3, 15];
        request.extend(b"fixture.invalid");
        request.extend(7u16.to_be_bytes());
        stream.write_all(&request).await?;
        let mut response = [0; 10];
        stream.read_exact(&mut response).await?;
        stream.write_all(b"socks-fixture").await?;
        let mut bytes = [0; 13];
        stream.read_exact(&mut bytes).await?;
        forward.stop();
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(
            greeting == [5, 0] && response[1] == 0 && &bytes == b"socks-fixture",
        )
    };
    checks.insert(
        "dynamicForward".into(),
        json!(
            tokio::time::timeout(Duration::from_secs(5), socks)
                .await
                .is_ok_and(|r| r.unwrap_or(false))
        ),
    );
    let remote = async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let port = listener.local_addr()?.port();
        let cancel = CancellationToken::new();
        let _cleanup = cancel.clone().drop_guard();
        let echo = tokio::spawn(async move {
            tokio::select! { () = cancel.cancelled() => {}, _ = async {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let mut bytes = [0; 14];
                    if stream.read_exact(&mut bytes).await.is_ok() { let _ = stream.write_all(&bytes).await; }
                }
            } => {} }
        });
        let forward = session
            .remote_forward(RemoteForward {
                bind_address: "127.0.0.1".into(),
                bind_port: 0,
                dest_host: "127.0.0.1".into(),
                dest_port: port,
            })
            .await?;
        let result = session.exec("ohhatoba-remote-probe", None).await;
        forward.stop();
        echo.abort();
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(
            result.is_ok_and(|(status, bytes)| status == 0 && bytes == b"remote-ok"),
        )
    };
    checks.insert(
        "remoteForward".into(),
        json!(
            tokio::time::timeout(Duration::from_secs(8), remote)
                .await
                .is_ok_and(|r| r.unwrap_or(false))
        ),
    );
    session.disconnect().await;
    for (name, passed) in http_checks().await {
        checks.insert(name, json!(passed));
    }

    json!({"checks":checks, "sftpStage":file_stage, "sftpErrno":file_errno})
}

async fn http_checks() -> Vec<(String, bool)> {
    use hatoba_ai::{
        chat::{self, ChatRequest, StreamEvent},
        provider::{AuthHeader, ModelSpec, Protocol, ProviderConfig},
    };
    let http = hatoba_ai::provider::http_client();
    let provider = ProviderConfig {
        protocol: Protocol::ChatCompletions,
        base_url: "http://127.0.0.1:28080/v1".into(),
        api_key: String::new().into(),
        auth_header: AuthHeader::Authorization,
        chatgpt: None,
    };
    let cancel = CancellationToken::new();
    let models = tokio::time::timeout(
        Duration::from_secs(5),
        hatoba_ai::models::list_models_unfilled(&http, &provider, &cancel),
    )
    .await
    .is_ok_and(|r| r.is_ok_and(|models| models.iter().any(|m| m.id == "generated-fixture")));
    let model = ModelSpec::new("generated-fixture");
    let request = ChatRequest {
        provider_id: "fixture",
        model: &model,
        system: "Generated diagnostic data only",
        tools: &[],
        entries: &[],
        effort: None,
    };
    let mut received = String::new();
    let stream = tokio::time::timeout(
        Duration::from_secs(5),
        chat::stream_chat(&http, &provider, &request, &cancel, &mut |event| {
            if let StreamEvent::Text(text) = event {
                received.push_str(&text);
            }
        }),
    )
    .await;
    let ai_stream = stream.is_ok_and(|r| r.is_ok_and(|answer| answer.text == "generated 中文 🌊"))
        && received == "generated 中文 🌊";
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    let ai_cancel = matches!(
        chat::stream_chat(&http, &provider, &request, &cancelled, &mut |_| {}).await,
        Err(hatoba_ai::AiError::Cancelled)
    );
    let mcp = async {
        use hatoba_ai::mcp::{McpConnection, McpTransportConfig};
        let config = McpTransportConfig::Http {
            url: "http://127.0.0.1:28080/mcp".into(),
            headers: vec![],
        };
        let conn = McpConnection::connect(&config, &http, Duration::from_secs(5)).await?;
        let tools = conn.list_tools().await?;
        let result = conn
            .call_tool(
                "echo",
                json!({"text":"generated 中文 🌊"})
                    .as_object()
                    .unwrap()
                    .clone(),
                &cancel,
                Duration::from_secs(5),
            )
            .await?;
        conn.shutdown().await;
        Ok::<_, hatoba_ai::mcp::McpError>(
            tools.iter().any(|t| t.name == "echo")
                && !result.is_error
                && result.content == "generated 中文 🌊"
                && conn.is_closed(),
        )
    };
    let mcp_pass = tokio::time::timeout(Duration::from_secs(8), mcp)
        .await
        .is_ok_and(|r| r.unwrap_or(false));
    let oauth = async {
        use hatoba_ai::chatgpt::{
            Endpoints,
            oauth::{SignIn, SignInRequest},
        };
        let endpoints = Endpoints::at("http://127.0.0.1:28080").unwrap();
        let request = SignInRequest {
            host_id: uuid::Uuid::new_v4().to_string(),
            ..Default::default()
        };
        let sign_in = SignIn::start(&endpoints, &request).await?;
        let state = sign_in
            .url()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into_owned();
        let pkce = sign_in
            .url()
            .query_pairs()
            .any(|(k, v)| k == "code_challenge_method" && v == "S256");
        let redirect = sign_in.redirect_uri().to_owned();
        let callbacks = async {
            let wrong = http
                .get(format!(
                    "{redirect}?state=wrong-generated-state&error=access_denied"
                ))
                .send()
                .await?;
            let correct = http
                .get(format!("{redirect}?state={state}&error=access_denied"))
                .send()
                .await?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(
                wrong.status().as_u16() == 400 && correct.status().is_success(),
            )
        };
        let (result, callbacks) = tokio::join!(sign_in.finish(&http, &cancel), callbacks);
        let denied = matches!(result, Err(hatoba_ai::AiError::SignIn(message)) if message.contains("cancelled in the browser"));
        let retry = SignIn::start(&endpoints, &request).await?;
        let cancelled = CancellationToken::new();
        cancelled.cancel();
        let cancelled = matches!(
            retry.finish(&http, &cancelled).await,
            Err(hatoba_ai::AiError::Cancelled)
        );
        Ok::<_, hatoba_ai::AiError>(pkce && denied && callbacks.unwrap_or(false) && cancelled)
    };
    let oauth_pass = tokio::time::timeout(Duration::from_secs(8), oauth)
        .await
        .is_ok_and(|r| r.unwrap_or(false));
    [
        ("aiHttpModels", models),
        ("aiStreamingUnicodeAndCancellation", ai_stream && ai_cancel),
        ("mcpHttpInitializeListCallAndShutdown", mcp_pass),
        ("oauthPkceStateCallbackAndCancellation", oauth_pass),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v))
    .collect()
}
