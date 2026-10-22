//! Protected vault keys use a separate, authentication-bound Asset alias.
//! The device encryption key never substitutes for biometric authentication.
use crate::{
    error::{AppError, AppResult, ErrorCode},
    host::AppHandle,
};
use hatoba_core::{crypto::Key32, platform::SecretStore};
use std::sync::atomic::{AtomicBool, Ordering};

static AVAILABLE: AtomicBool = AtomicBool::new(false);
pub fn configure(available: bool) {
    AVAILABLE.store(available, Ordering::Relaxed);
}
pub fn available() -> bool {
    AVAILABLE.load(Ordering::Relaxed)
}
fn unavailable() -> AppError {
    AppError::new(
        ErrorCode::BiometricUnavailable,
        "OS biometric authentication is unavailable or was cancelled",
    )
}

#[cfg(target_env = "ohos")]
unsafe extern "C" {
    fn hatoba_biometric_exists() -> i32;
    fn hatoba_biometric_remove() -> i32;
    fn hatoba_biometric_store(key: *const u8) -> i32;
    fn hatoba_biometric_prepare(out: *mut u8) -> i32;
    fn hatoba_biometric_finish(challenge: *const u8) -> i32;
    fn hatoba_biometric_read(
        challenge: *const u8,
        token: *const u8,
        length: u32,
        out: *mut u8,
    ) -> i32;
}
pub fn enrolled(_: &AppHandle, _: &dyn SecretStore) -> bool {
    #[cfg(target_env = "ohos")]
    {
        unsafe { hatoba_biometric_exists() == 0 }
    }
    #[cfg(not(target_env = "ohos"))]
    {
        false
    }
}
pub fn remove(_: &AppHandle, _: &dyn SecretStore) -> AppResult<()> {
    #[cfg(target_env = "ohos")]
    {
        if unsafe { hatoba_biometric_remove() } == 0 {
            return Ok(());
        }
    }
    Err(unavailable())
}
pub async fn enroll(app: &AppHandle, secrets: &dyn SecretStore, key: &Key32) -> AppResult<()> {
    if !available() {
        return Err(unavailable());
    }
    #[cfg(target_env = "ohos")]
    {
        if unsafe { hatoba_biometric_store(key.as_ptr()) } != 0 {
            return Err(unavailable());
        }
        // Verify this device can actually open the protected asset before advertising enrollment.
        match unlock(app, secrets).await {
            Ok(verified) if verified.as_slice() == key.as_slice() => return Ok(()),
            _ => {
                let _ = remove(app, secrets);
            }
        }
    }
    #[cfg(not(target_env = "ohos"))]
    let _ = (app, secrets, key);
    Err(unavailable())
}
pub async fn unlock(app: &AppHandle, _: &dyn SecretStore) -> AppResult<Key32> {
    #[cfg(target_env = "ohos")]
    {
        use zeroize::Zeroizing;
        struct Challenge(Zeroizing<[u8; 32]>);
        impl Drop for Challenge {
            fn drop(&mut self) {
                unsafe {
                    hatoba_biometric_finish(self.0.as_ptr());
                }
            }
        }
        let mut challenge = Challenge(Zeroizing::new([0; 32]));
        let status = unsafe { hatoba_biometric_prepare(challenge.0.as_mut_ptr()) };
        if status == 24000002 {
            return Err(AppError::new(
                ErrorCode::BiometricStale,
                "Biometric enrollment is no longer available",
            ));
        }
        if status != 0 {
            return Err(unavailable());
        }
        let mut result = app
            .service(
                "biometric_auth",
                serde_json::json!({"challenge":challenge.0.as_slice()}),
            )
            .await
            .map_err(|_| unavailable())?;
        let token: Zeroizing<Vec<u8>> =
            Zeroizing::new(serde_json::from_value(result.take()).map_err(|_| unavailable())?);
        if token.is_empty() || token.len() > 1024 {
            return Err(unavailable());
        }
        let mut key = hatoba_core::crypto::random_key()?;
        let status = unsafe {
            hatoba_biometric_read(
                challenge.0.as_ptr(),
                token.as_ptr(),
                token.len() as u32,
                key.as_mut_ptr(),
            )
        };
        if status == 0 {
            return Ok(key);
        }
        if status == 24000002 {
            return Err(AppError::new(
                ErrorCode::BiometricStale,
                "Biometric enrollment changed",
            ));
        }
    }
    #[cfg(not(target_env = "ohos"))]
    let _ = app;
    Err(unavailable())
}
