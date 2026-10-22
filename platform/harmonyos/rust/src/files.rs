//! Opaque grants from the native document picker. Web paths can never name sandbox files.
use hatoba_runtime::error::{AppError, AppResult};
use serde_json::Value;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    os::fd::{BorrowedFd, OwnedFd},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

struct Grant {
    path: PathBuf,
    descriptor: OwnedFd,
    name: String,
    write: bool,
}
pub struct Grants {
    directory: PathBuf,
    entries: Mutex<HashMap<String, Grant>>,
    closed: Arc<AtomicBool>,
}
impl Grants {
    pub fn cleanup_stale(cache: &Path) -> AppResult<()> {
        for entry in fs::read_dir(cache)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Some(id) = name.strip_prefix("hatoba-grants-")
                && uuid::Uuid::parse_str(id).is_ok()
            {
                let kind = entry.file_type()?;
                if kind.is_dir() && !kind.is_symlink() {
                    fs::remove_dir_all(entry.path())?;
                }
            }
        }
        Ok(())
    }
    pub fn release(&self, path: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let handle = entries
            .iter()
            .find(|(handle, grant)| handle.as_str() == path || grant.path == Path::new(path))
            .map(|(handle, _)| handle.clone());
        if let Some(handle) = handle
            && let Some(grant) = entries.remove(&handle)
        {
            if grant.write {
                let _ = fs::remove_file(&grant.path);
            }
            if let Some(id) = handle
                .strip_prefix("grant://")
                .and_then(|h| h.split('/').next())
            {
                let _ = fs::remove_dir(self.directory.join(id));
            }
        }
    }
    pub fn new(cache: &Path) -> AppResult<Self> {
        if !cache.is_absolute() {
            return Err(AppError::invalid(
                "cacheDir",
                "Expected native cache directory",
            ));
        }
        let directory = cache.join(format!("hatoba-grants-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory)?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        Ok(Self {
            directory,
            entries: Mutex::new(HashMap::new()),
            closed: Arc::new(AtomicBool::new(false)),
        })
    }
    /// # Safety
    /// The descriptor must be open for the entire call, and obtained by native user selection.
    pub unsafe fn register(&self, fd: i32, filename: &str, write: bool) -> AppResult<String> {
        if self.closed.load(Ordering::Acquire)
            || fd < 0
            || filename.is_empty()
            || filename.len() > 255
            || filename == "."
            || filename == ".."
            || filename.contains(['/', '\\', '\0'])
        {
            return Err(AppError::invalid("file", "Invalid document selection"));
        }
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if entries.len() >= 128 {
            return Err(AppError::invalid("file", "Too many open document grants"));
        }
        let descriptor = unsafe { BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()?;
        let token = uuid::Uuid::new_v4().to_string();
        let directory = self.directory.join(&token);
        fs::create_dir(&directory)?;
        let path = if !write {
            use std::os::fd::AsRawFd;
            #[cfg(target_os = "macos")]
            let fd_root = "/dev/fd";
            #[cfg(not(target_os = "macos"))]
            let fd_root = "/proc/self/fd";
            PathBuf::from(format!("{fd_root}/{}", descriptor.as_raw_fd()))
        } else {
            directory.join(filename)
        };
        let handle = format!("grant://{token}/{filename}");
        entries.insert(
            handle.clone(),
            Grant {
                path,
                descriptor,
                name: filename.into(),
                write,
            },
        );
        Ok(handle)
    }
    pub fn resolve(&self, handle: &str, write: bool) -> AppResult<String> {
        if self.closed.load(Ordering::Acquire) {
            return Err(AppError::invalid("path", "Document grant expired"));
        }
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let grant = entries
            .get(handle)
            .filter(|g| g.write == write)
            .ok_or_else(|| {
                AppError::invalid("path", "Select this document using the native picker")
            })?;
        Ok(grant.path.to_string_lossy().into_owned())
    }
    pub fn prepare(&self, command: &str, args: &mut Value) -> AppResult<Option<String>> {
        let (field, write, commit) = match command {
            "vault_export_backup" | "skill_export" => (Some("path"), true, true),
            "save_text_file" => (Some("path"), true, false),
            "sftp_download" => (Some("localPath"), true, false),
            "sftp_upload" => (Some("localPath"), false, false),
            "skill_import" | "skill_import_preview" | "hosts_import" | "hosts_import_preview" => {
                (Some("path"), false, false)
            }
            _ => (None, false, false),
        };
        let mut output = None;
        if let Some(field) = field {
            let handle = args
                .get(field)
                .and_then(Value::as_str)
                .ok_or_else(|| AppError::invalid(field, "Select a document first"))?;
            let path = self.resolve(handle, write)?;
            if commit {
                output = Some(path.clone());
            }
            args[field] = Value::String(path);
        }
        if command == "key_import"
            && let Some(path) = args.pointer("/input/path").and_then(Value::as_str)
        {
            let path = self.resolve(path, false)?;
            args["input"]["path"] = Value::String(path);
        }
        if command == "hosts_import" {
            let held = Self::held_import_keys(args)?;
            if let Some(keys) = args.get_mut("keyFiles").and_then(Value::as_array_mut) {
                for key in keys {
                    let path = key
                        .as_str()
                        .ok_or_else(|| AppError::invalid("keyFiles", "Invalid key selection"))?;
                    if held.contains(path) {
                        continue;
                    }
                    *key = Value::String(self.resolve(path, false)?);
                }
            }
        }
        Ok(output)
    }
    fn held_import_keys(args: &Value) -> AppResult<std::collections::HashSet<String>> {
        let source = serde_json::from_value(args["source"].clone())
            .map_err(|_| AppError::invalid("source", "Invalid import source"))?;
        let path = args["path"]
            .as_str()
            .ok_or_else(|| AppError::invalid("path", "Select an import file"))?;
        Ok(hatoba_runtime::import::read(source, Some(Path::new(path)))?
            .key_files
            .into_keys()
            .collect())
    }
    pub fn review_import_preview(&self, args: &Value, result: &mut Value) -> AppResult<()> {
        let held = Self::held_import_keys(args)?;
        if let Some(candidates) = result["candidates"].as_array_mut() {
            for candidate in candidates {
                if !candidate["identity_file"]
                    .as_str()
                    .is_some_and(|path| held.contains(path))
                {
                    // Foreign filesystem paths require a separate key selection on this device.
                    candidate["identity_file_found"] = Value::Bool(false);
                }
            }
        }
        Ok(())
    }
    pub fn public_value(&self, value: &mut Value) {
        fn replace(value: &mut Value, entries: &HashMap<String, Grant>) {
            match value {
                Value::String(text) => {
                    if let Some((handle, _)) = entries.iter().find(|(_, grant)| {
                        grant.path.as_os_str() == std::ffi::OsStr::new(text.as_str())
                    }) {
                        *text = handle.clone();
                    }
                }
                Value::Array(list) => list.iter_mut().for_each(|v| replace(v, entries)),
                Value::Object(map) => map.values_mut().for_each(|v| replace(v, entries)),
                _ => {}
            }
        }
        replace(
            value,
            &self.entries.lock().unwrap_or_else(|e| e.into_inner()),
        );
    }
    pub fn input_name(&self, path: &str) -> AppResult<String> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries
            .values()
            .find(|g| !g.write && g.path == Path::new(path))
            .map(|g| g.name.clone())
            .ok_or_else(|| AppError::invalid("path", "No read grant for document"))
    }
    pub async fn write_text(&self, path: &str, contents: &str) -> AppResult<()> {
        if !["txt", "md", "json"].contains(
            &Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or(""),
        ) {
            return Err(AppError::invalid("path", "Choose a text file"));
        }
        self.ensure_output(path)?;
        tokio::fs::write(path, contents).await?;
        self.commit(path).await
    }
    fn ensure_output(&self, path: &str) -> AppResult<OwnedFd> {
        if self.closed.load(Ordering::Acquire) {
            return Err(AppError::invalid("path", "Document grant expired"));
        }
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries
            .values()
            .find(|g| g.write && g.path == Path::new(path))
            .ok_or_else(|| AppError::invalid("path", "No write grant for document"))?
            .descriptor
            .try_clone()
            .map_err(Into::into)
    }
    pub async fn commit(&self, path: &str) -> AppResult<()> {
        let fd = self.ensure_output(path)?;
        let path = PathBuf::from(path);
        let closed = self.closed.clone();
        let cancel = tokio_util::sync::CancellationToken::new();
        let _cancel_on_drop = cancel.clone().drop_guard();
        tokio::task::spawn_blocking(move || -> AppResult<()> {
            let mut source = File::open(path)?;
            let mut target = File::from(fd);
            target.seek(SeekFrom::Start(0))?;
            target.set_len(0)?;
            let mut buffer = vec![0; 64 * 1024];
            loop {
                if closed.load(Ordering::Acquire) || cancel.is_cancelled() {
                    return Err(AppError::internal("Document export was cancelled"));
                }
                let length = source.read(&mut buffer)?;
                if length == 0 {
                    break;
                }
                target.write_all(&buffer[..length])?;
            }
            target.sync_all()?;
            Ok(())
        })
        .await
        .map_err(|_| AppError::internal("Document export failed"))?
    }
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
}
impl Drop for Grants {
    fn drop(&mut self) {
        self.close();
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::fd::AsRawFd;
    fn area() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hatoba-grant-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        dir
    }
    #[test]
    fn stale_cleanup_removes_only_owned_grant_directories() {
        let dir = area();
        let stale = dir.join(format!("hatoba-grants-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&stale).unwrap();
        fs::write(stale.join("plaintext"), b"generated only").unwrap();
        let other = dir.join("other-cache");
        fs::create_dir(&other).unwrap();
        let link = dir.join(format!("hatoba-grants-{}", uuid::Uuid::new_v4()));
        std::os::unix::fs::symlink(&other, &link).unwrap();
        Grants::cleanup_stale(&dir).unwrap();
        assert!(!stale.exists());
        assert!(other.is_dir());
        assert!(link.is_symlink());
        fs::remove_dir_all(dir).unwrap();
    }
    #[tokio::test]
    async fn only_explicit_picker_grants_can_read_or_write_and_survive_original_fd_close() {
        let dir = area();
        let grants = Grants::new(&dir).unwrap();
        let input = dir.join("input.txt");
        fs::write(&input, b"generated input").unwrap();
        let file = File::open(&input).unwrap();
        let read = unsafe { grants.register(file.as_raw_fd(), "input.txt", false) }.unwrap();
        drop(file);
        let path = grants.resolve(&read, false).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"generated input");
        assert!(grants.resolve(&read, true).is_err());
        assert!(grants.resolve(input.to_str().unwrap(), false).is_err());
        assert!(grants.resolve("grant://invented/input.txt", false).is_err());
        let target = dir.join("selected.txt");
        let file = File::create(&target).unwrap();
        let write = unsafe { grants.register(file.as_raw_fd(), "selected.txt", true) }.unwrap();
        drop(file);
        let output = grants.resolve(&write, true).unwrap();
        grants.write_text(&output, "中文 🌊\n").await.unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "中文 🌊\n");
        let mut response = serde_json::json!({"path":path});
        grants.public_value(&mut response);
        assert_eq!(response["path"], read);
        let mut forged = serde_json::json!({"path":input});
        assert!(grants.prepare("hosts_import_preview", &mut forged).is_err());
        grants.close();
        assert!(grants.resolve(&read, false).is_err());
        assert!(grants.commit(&output).await.is_err());
        drop(grants);
        assert!(input.exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
