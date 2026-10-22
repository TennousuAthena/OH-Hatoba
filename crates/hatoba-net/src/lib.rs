//! One HTTP trust-store boundary shared by sync, providers, MCP and web tools.
//! Desktop retains reqwest's platform verifier. OHOS roots come only from Certificate Manager.
#[cfg(target_env = "ohos")]
use std::sync::{OnceLock, RwLock};
#[cfg(target_env = "ohos")]
static ROOTS: OnceLock<RwLock<Vec<reqwest::Certificate>>> = OnceLock::new();

pub fn client_builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    #[cfg(target_env = "ohos")]
    {
        let roots = ROOTS
            .get_or_init(|| RwLock::new(Vec::new()))
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        // An unconfigured store has zero trusted roots and fails TLS closed. Never disable
        // certificate, expiry or hostname validation to make a port appear to work.
        builder.tls_certs_only(roots)
    }
    #[cfg(not(target_env = "ohos"))]
    {
        builder
    }
}

#[cfg(target_env = "ohos")]
pub fn configure_system_ca(path: &std::path::Path) -> Result<usize, &'static str> {
    if !path.is_absolute() || !path.is_dir() {
        return Err("System certificate directory is unavailable");
    }
    let loaded = rustls_native_certs::load_certs_from_paths(None, Some(path));
    let certificates: Vec<_> = loaded
        .certs
        .into_iter()
        .filter_map(|der| reqwest::Certificate::from_der(&der).ok())
        .collect();
    if certificates.is_empty() {
        return Err("System certificate store is empty");
    }
    let count = certificates.len();
    *ROOTS
        .get_or_init(|| RwLock::new(Vec::new()))
        .write()
        .unwrap_or_else(|e| e.into_inner()) = certificates;
    Ok(count)
}
