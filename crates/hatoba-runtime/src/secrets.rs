//! Arbitrarily sized native secrets sealed with a device key held by the OS asset store.
//! Biometric vault keys require a separate authentication-protected asset, never this KEK.
use hatoba_core::{
    Error, Result,
    crypto::{self, Envelope, Key32},
    platform::{SecretStore, secret_keys},
};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
use zeroize::Zeroizing;

pub struct EnvelopeSecretStore {
    directory: PathBuf,
    key: Key32,
    serial: Mutex<()>,
}
impl EnvelopeSecretStore {
    pub fn new(directory: &Path, key: Key32) -> Result<Self> {
        fs::create_dir_all(directory)?;
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(invalid());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        }
        Ok(Self {
            directory: directory.to_owned(),
            key,
            serial: Mutex::new(()),
        })
    }
    fn location(&self, key: &str) -> Result<(PathBuf, Vec<u8>)> {
        if key.is_empty() || key.len() > 256 || key == secret_keys::BIOMETRIC_VAULT_KEY {
            return Err(invalid());
        }
        let aad = serde_json::to_vec(&("oh-hatoba/native-secret/v1", key))?;
        Ok((self.directory.join(crypto::sha256_hex(key.as_bytes())), aad))
    }
    fn regular(path: &Path) -> Result<bool> {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() && !meta.file_type().is_symlink() => Ok(true),
            Ok(_) => Err(invalid()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}
fn invalid() -> Error {
    Error::Io(std::io::Error::other(
        "native secret store rejected the operation",
    ))
}
impl SecretStore for EnvelopeSecretStore {
    fn get(&self, key: &str) -> Result<Option<Zeroizing<String>>> {
        let _guard = self.serial.lock().map_err(|_| Error::Poisoned)?;
        let (path, aad) = self.location(key)?;
        if !Self::regular(&path)? {
            return Ok(None);
        }
        let mut encoded = Vec::new();
        fs::File::open(path)?
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut encoded)?;
        if encoded.len() > 2 * 1024 * 1024 {
            return Err(invalid());
        }
        let envelope: Envelope = serde_json::from_slice(&encoded)?;
        let plaintext = crypto::open(&self.key, &aad, &envelope)?;
        let value = std::str::from_utf8(&plaintext).map_err(|_| invalid())?;
        Ok(Some(Zeroizing::new(value.to_owned())))
    }
    fn set(&self, key: &str, value: &str) -> Result<()> {
        let _guard = self.serial.lock().map_err(|_| Error::Poisoned)?;
        if value.len() > 1024 * 1024 {
            return Err(invalid());
        }
        let (path, aad) = self.location(key)?;
        Self::regular(&path)?;
        let encoded = serde_json::to_vec(&crypto::seal(&self.key, &aad, value.as_bytes())?)?;
        let temp = self
            .directory
            .join(format!(".{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> Result<()> {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temp)?;
            file.write_all(&encoded)?;
            file.sync_all()?;
            fs::rename(&temp, &path)?;
            fs::File::open(&self.directory)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
    fn delete(&self, key: &str) -> Result<()> {
        let _guard = self.serial.lock().map_err(|_| Error::Poisoned)?;
        let (path, _) = self.location(key)?;
        if Self::regular(&path)? {
            fs::remove_file(path)?;
            fs::File::open(&self.directory)?.sync_all()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn directory() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("oh-hatoba-secret-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        path
    }
    #[test]
    fn persists_large_values_without_plaintext_and_rejects_record_substitution() {
        let path = directory();
        let key = crypto::random_key().unwrap();
        let store = EnvelopeSecretStore::new(&path, key.clone()).unwrap();
        let value = "test-token-fixture".repeat(400);
        store.set("sync/session", &value).unwrap();
        let file = store.location("sync/session").unwrap().0;
        assert!(
            !fs::read_to_string(&file)
                .unwrap()
                .contains("test-token-fixture")
        );
        drop(store);
        let reopened = EnvelopeSecretStore::new(&path, key).unwrap();
        assert_eq!(
            reopened.get("sync/session").unwrap().unwrap().as_str(),
            value
        );
        let other = reopened.location("sync/d1-api-token").unwrap().0;
        fs::copy(file, other).unwrap();
        assert!(reopened.get("sync/d1-api-token").is_err());
        assert!(
            reopened
                .set(secret_keys::BIOMETRIC_VAULT_KEY, "fixture")
                .is_err()
        );
        reopened.delete("sync/session").unwrap();
        reopened.delete("sync/session").unwrap();
        assert!(reopened.get("sync/session").unwrap().is_none());
        fs::remove_dir_all(path).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn refuses_symlink_records() {
        let path = directory();
        let store = EnvelopeSecretStore::new(&path, crypto::random_key().unwrap()).unwrap();
        let record = store.location("sync/session").unwrap().0;
        let target = path.join("outside");
        fs::write(&target, "fixture").unwrap();
        std::os::unix::fs::symlink(&target, &record).unwrap();
        assert!(store.get("sync/session").is_err());
        assert!(store.set("sync/session", "changed").is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "fixture");
        fs::remove_dir_all(path).unwrap();
    }
}
