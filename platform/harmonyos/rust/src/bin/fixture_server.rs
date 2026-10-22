//! Loopback-only disposable SSH/SFTP fixture. Never executes commands or opens host files.
//! Requires an explicit test-tools feature and is never included in the HAP.
use russh::{
    Channel, ChannelId, Pty,
    server::{Auth, ChannelOpenHandle, Msg, Session},
};
use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode,
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

type Files = Arc<Mutex<HashMap<String, Vec<u8>>>>;
fn path(value: &str) -> Result<String, StatusCode> {
    if value.len() > 512 || value.contains("..") || value.contains('\\') {
        return Err(StatusCode::PermissionDenied);
    }
    Ok(format!(
        "/{}",
        value.trim_start_matches('/').trim_start_matches("./")
    ))
}
fn status(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: String::new(),
        language_tag: String::new(),
    }
}
fn attrs(size: usize, dir: bool) -> FileAttributes {
    FileAttributes {
        size: Some(size as u64),
        permissions: Some(if dir { 0o040700 } else { 0o100600 }),
        ..FileAttributes::default()
    }
}
struct Sftp {
    files: Files,
    handles: HashMap<String, String>,
    listed: bool,
}
impl russh_sftp::server::Handler for Sftp {
    type Error = StatusCode;
    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }
    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        _: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        let filename = path(&filename)?;
        let mut files = self.files.lock().unwrap();
        if flags.contains(OpenFlags::EXCLUDE) && files.contains_key(&filename) {
            return Err(StatusCode::Failure);
        }
        if !files.contains_key(&filename) && !flags.contains(OpenFlags::CREATE) {
            return Err(StatusCode::NoSuchFile);
        }
        if flags.contains(OpenFlags::TRUNCATE) || !files.contains_key(&filename) {
            files.insert(filename.clone(), Vec::new());
        }
        let handle = uuid::Uuid::new_v4().to_string();
        self.handles.insert(handle.clone(), filename);
        Ok(Handle { id, handle })
    }
    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.handles.remove(&handle);
        Ok(status(id))
    }
    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        let name = self.handles.get(&handle).ok_or(StatusCode::Failure)?;
        let files = self.files.lock().unwrap();
        let file = files.get(name).ok_or(StatusCode::NoSuchFile)?;
        let start = usize::try_from(offset).map_err(|_| StatusCode::Failure)?;
        if start >= file.len() {
            return Err(StatusCode::Eof);
        }
        Ok(Data {
            id,
            data: file[start..file.len().min(start + (len as usize).min(64 * 1024))].to_vec(),
        })
    }
    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let start = usize::try_from(offset).map_err(|_| StatusCode::Failure)?;
        let end = start
            .checked_add(data.len())
            .filter(|n| *n <= 2 * 1024 * 1024)
            .ok_or(StatusCode::PermissionDenied)?;
        let name = self.handles.get(&handle).ok_or(StatusCode::Failure)?;
        let mut files = self.files.lock().unwrap();
        let file = files.get_mut(name).ok_or(StatusCode::NoSuchFile)?;
        file.resize(file.len().max(end), 0);
        file[start..end].copy_from_slice(&data);
        Ok(status(id))
    }
    async fn stat(&mut self, id: u32, filename: String) -> Result<Attrs, Self::Error> {
        let filename = path(&filename)?;
        if filename == "/" {
            return Ok(Attrs {
                id,
                attrs: attrs(0, true),
            });
        }
        let files = self.files.lock().unwrap();
        let file = files.get(&filename).ok_or(StatusCode::NoSuchFile)?;
        Ok(Attrs {
            id,
            attrs: attrs(file.len(), false),
        })
    }
    async fn lstat(&mut self, id: u32, filename: String) -> Result<Attrs, Self::Error> {
        self.stat(id, filename).await
    }
    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        self.stat(
            id,
            self.handles
                .get(&handle)
                .ok_or(StatusCode::Failure)?
                .clone(),
        )
        .await
    }
    async fn fsetstat(
        &mut self,
        id: u32,
        _: String,
        _: FileAttributes,
    ) -> Result<Status, Self::Error> {
        Ok(status(id))
    }
    async fn setstat(
        &mut self,
        id: u32,
        _: String,
        _: FileAttributes,
    ) -> Result<Status, Self::Error> {
        Ok(status(id))
    }
    async fn realpath(&mut self, id: u32, _: String) -> Result<Name, Self::Error> {
        Ok(Name {
            id,
            files: vec![File::new("/", attrs(0, true))],
        })
    }
    async fn opendir(&mut self, id: u32, filename: String) -> Result<Handle, Self::Error> {
        if path(&filename)? != "/" {
            return Err(StatusCode::NoSuchFile);
        }
        self.listed = false;
        Ok(Handle {
            id,
            handle: "/".into(),
        })
    }
    async fn readdir(&mut self, id: u32, _: String) -> Result<Name, Self::Error> {
        if self.listed {
            return Err(StatusCode::Eof);
        }
        self.listed = true;
        Ok(Name {
            id,
            files: self
                .files
                .lock()
                .unwrap()
                .iter()
                .map(|(name, bytes)| {
                    File::new(name.trim_start_matches('/'), attrs(bytes.len(), false))
                })
                .collect(),
        })
    }
    async fn rename(&mut self, id: u32, old: String, new: String) -> Result<Status, Self::Error> {
        let mut files = self.files.lock().unwrap();
        let data = files.remove(&path(&old)?).ok_or(StatusCode::NoSuchFile)?;
        files.insert(path(&new)?, data);
        Ok(status(id))
    }
    async fn remove(&mut self, id: u32, name: String) -> Result<Status, Self::Error> {
        self.files.lock().unwrap().remove(&path(&name)?);
        Ok(status(id))
    }
}
struct Handler {
    channels: HashMap<ChannelId, Channel<Msg>>,
    shells: HashSet<ChannelId>,
    files: Files,
    forwards: HashMap<u32, tokio_util::sync::CancellationToken>,
}
impl Drop for Handler {
    fn drop(&mut self) {
        for token in self.forwards.values() {
            token.cancel();
        }
    }
}
impl russh::server::Handler for Handler {
    type Error = russh::Error;
    async fn auth_password(&mut self, user: &str, password: &str) -> Result<Auth, Self::Error> {
        Ok(
            if user == "fixture" && password == "generated-diagnostic-only" {
                Auth::Accept
            } else {
                Auth::reject()
            },
        )
    }
    async fn channel_open_session(
        &mut self,
        channel: Channel<Msg>,
        reply: ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        self.channels.insert(channel.id(), channel);
        reply.accept().await;
        Ok(())
    }
    async fn pty_request(
        &mut self,
        channel: ChannelId,
        _: &str,
        _: u32,
        _: u32,
        _: u32,
        _: u32,
        _: &[(Pty, u32)],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        session.channel_success(channel)?;
        Ok(())
    }
    async fn shell_request(
        &mut self,
        channel: ChannelId,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        self.shells.insert(channel);
        session.channel_success(channel)?;
        Ok(())
    }
    async fn data(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if self.shells.contains(&channel) {
            session.data(channel, data.to_vec())?;
        }
        Ok(())
    }
    async fn exec_request(
        &mut self,
        channel: ChannelId,
        command: &[u8],
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if command == b"ohhatoba-remote-probe" {
            let Some(port) = self.forwards.keys().next().copied() else {
                session.channel_failure(channel)?;
                return Ok(());
            };
            let handle = session.handle();
            session.channel_success(channel)?;
            tokio::spawn(async move {
                let result = tokio::time::timeout(Duration::from_secs(5), async {
                    let mut stream =
                        tokio::net::TcpStream::connect(("127.0.0.1", port as u16)).await?;
                    stream.write_all(b"remote-fixture").await?;
                    let mut bytes = [0; 14];
                    stream.read_exact(&mut bytes).await?;
                    Ok::<_, std::io::Error>(bytes == *b"remote-fixture")
                })
                .await
                .is_ok_and(|value| value.unwrap_or(false));
                let _ = handle
                    .data(
                        channel,
                        if result {
                            b"remote-ok".to_vec()
                        } else {
                            b"remote-failed".to_vec()
                        },
                    )
                    .await;
                let _ = handle
                    .exit_status_request(channel, if result { 0 } else { 1 })
                    .await;
                let _ = handle.eof(channel).await;
                let _ = handle.close(channel).await;
            });
            return Ok(());
        }
        session.channel_success(channel)?;
        session.data(
            channel,
            if command == b"ohhatoba-diagnostic" {
                b"fixture-exec-ok\n".to_vec()
            } else {
                b"Linux\n".to_vec()
            },
        )?;
        session.exit_status_request(channel, 0)?;
        session.eof(channel)?;
        session.close(channel)?;
        Ok(())
    }
    async fn subsystem_request(
        &mut self,
        id: ChannelId,
        name: &str,
        session: &mut Session,
    ) -> Result<(), Self::Error> {
        if name != "sftp" {
            session.channel_failure(id)?;
            return Ok(());
        }
        if let Some(channel) = self.channels.remove(&id) {
            session.channel_success(id)?;
            let files = self.files.clone();
            tokio::spawn(async move {
                russh_sftp::server::run(
                    channel.into_stream(),
                    Sftp {
                        files,
                        handles: HashMap::new(),
                        listed: false,
                    },
                )
                .await;
            });
        }
        Ok(())
    }
    async fn tcpip_forward(
        &mut self,
        address: &str,
        port: &mut u32,
        session: &mut Session,
    ) -> Result<bool, Self::Error> {
        if address != "127.0.0.1" || *port != 0 || !self.forwards.is_empty() {
            return Ok(false);
        }
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(value) => value,
            Err(_) => return Ok(false),
        };
        *port = u32::from(listener.local_addr().map_err(russh::Error::IO)?.port());
        let token = tokio_util::sync::CancellationToken::new();
        self.forwards.insert(*port, token.clone());
        let handle = session.handle();
        let bound = *port;
        tokio::spawn(async move {
            loop {
                let incoming = tokio::select! { biased; () = token.cancelled() => break, value = listener.accept() => value };
                let Ok((mut stream, peer)) = incoming else {
                    break;
                };
                let handle = handle.clone();
                let cancel = token.clone();
                tokio::spawn(async move {
                    let transfer = async {
                        if let Ok(channel) = handle
                            .channel_open_forwarded_tcpip(
                                "127.0.0.1",
                                bound,
                                "127.0.0.1",
                                u32::from(peer.port()),
                            )
                            .await
                        {
                            let _ = tokio::io::copy_bidirectional(
                                &mut stream,
                                &mut channel.into_stream(),
                            )
                            .await;
                        }
                    };
                    tokio::select! { () = cancel.cancelled() => {}, () = transfer => {} }
                });
            }
        });
        Ok(true)
    }
    async fn cancel_tcpip_forward(
        &mut self,
        address: &str,
        port: u32,
        _: &mut Session,
    ) -> Result<bool, Self::Error> {
        if address != "127.0.0.1" {
            return Ok(false);
        }
        if let Some(token) = self.forwards.remove(&port) {
            token.cancel();
            return Ok(true);
        }
        Ok(false)
    }
    async fn channel_open_direct_tcpip(
        &mut self,
        channel: Channel<Msg>,
        host: &str,
        port: u32,
        _: &str,
        _: u32,
        reply: ChannelOpenHandle,
        _: &mut Session,
    ) -> Result<(), Self::Error> {
        if host != "fixture.invalid" || port != 7 {
            return Ok(());
        }
        reply.accept().await;
        tokio::spawn(async move {
            let mut stream = channel.into_stream();
            let mut bytes = [0; 8192];
            while let Ok(n) = stream.read(&mut bytes).await {
                if n == 0 || stream.write_all(&bytes[..n]).await.is_err() {
                    break;
                }
            }
        });
        Ok(())
    }
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let key = russh::keys::PrivateKey::random(
        &mut russh::keys::key::safe_rng(),
        russh::keys::Algorithm::Ed25519,
    )?;
    let config = Arc::new(russh::server::Config {
        keys: vec![key],
        auth_rejection_time: Duration::from_millis(10),
        auth_rejection_time_initial: Some(Duration::ZERO),
        ..Default::default()
    });
    let files: Files = Arc::new(Mutex::new(HashMap::from([(
        "/fixture.bin".into(),
        (0..524_288).map(|n| (n % 251) as u8).collect(),
    )])));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:22222").await?;
    println!("Disposable loopback fixture ready on 22222 (no host shell or filesystem access)");
    loop {
        let (stream, _) = listener.accept().await?;
        let config = config.clone();
        let files = files.clone();
        tokio::spawn(async move {
            if let Ok(connection) = russh::server::run_stream(
                config,
                stream,
                Handler {
                    channels: HashMap::new(),
                    shells: HashSet::new(),
                    files,
                    forwards: HashMap::new(),
                },
            )
            .await
            {
                let _ = connection.await;
            }
        });
    }
}
