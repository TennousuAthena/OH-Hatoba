//! Native host boundary. Business algorithms stay in upstream crates; this crate owns only
//! HarmonyOS services and their C ABI. The probe is deliberately separate from any user vault.

use std::ffi::{CString, c_char};

use hatoba_core::crypto::{open, random_key, seal};
use serde::Serialize;

/// This version describes the native host protocol, independently of the upstream app version.
pub const HOST_PROTOCOL: u32 = 1;

#[derive(Debug, Serialize)]
pub struct ProbeCheck {
    pub name: &'static str,
    pub passed: bool,
}

#[derive(Debug, Serialize)]
pub struct ProbeReport {
    pub host_protocol: u32,
    pub upstream_version: &'static str,
    pub upstream_commit: String,
    pub architecture: &'static str,
    pub target_os: &'static str,
    pub target_env: &'static str,
    pub checks: Vec<ProbeCheck>,
}

impl ProbeReport {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed)
    }
}

/// Exercise actual upstream implementations without creating, reading or altering user data.
/// A successful host run is not evidence of an OHOS device run; target fields travel with results.
pub fn probe() -> ProbeReport {
    let crypto = (|| {
        let key = random_key()?;
        let envelope = seal(&key, b"oh-hatoba/probe/v1", b"OH-Hatoba probe")?;
        let decoded = open(&key, b"oh-hatoba/probe/v1", &envelope)?;
        Ok::<_, hatoba_core::Error>(
            decoded.as_slice() == b"OH-Hatoba probe"
                && open(&key, b"oh-hatoba/wrong-domain/v1", &envelope).is_err(),
        )
    })();
    let sqlite = hatoba_core::store::Store::open_in_memory().is_ok();
    let ssh_key = hatoba_ssh::generate_key(hatoba_ssh::GenerateKind::Ed25519, "probe", None)
        .and_then(|key| hatoba_ssh::parse_private_key(key.openssh_private.as_str(), None))
        .is_ok();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map(|runtime| runtime.block_on(async { tokio::task::yield_now().await }))
        .is_ok();
    let ai = hatoba_ai::provider::validate_base_url("https://example.com/v1").is_ok()
        && hatoba_ai::provider::validate_base_url("file:///private/secret").is_err();
    let baseline: serde_json::Value = serde_json::from_str(include_str!("../../upstream.json"))
        .expect("checked-in upstream baseline must be valid JSON");
    ProbeReport {
        host_protocol: HOST_PROTOCOL,
        upstream_version: env!("CARGO_PKG_VERSION"),
        upstream_commit: baseline["commit"].as_str().unwrap_or("unknown").into(),
        architecture: std::env::consts::ARCH,
        target_os: std::env::consts::OS,
        target_env: if cfg!(target_env = "ohos") {
            "ohos"
        } else {
            "host"
        },
        checks: vec![
            ProbeCheck {
                name: "random_aead_and_tamper_rejection",
                passed: crypto.unwrap_or(false),
            },
            ProbeCheck {
                name: "sqlite_schema",
                passed: sqlite,
            },
            ProbeCheck {
                name: "ssh_ed25519_roundtrip",
                passed: ssh_key,
            },
            ProbeCheck {
                name: "tokio_runtime",
                passed: runtime,
            },
            ProbeCheck {
                name: "ai_provider_url_validation",
                passed: ai,
            },
        ],
    }
}

/// Returns owned UTF-8 JSON. No secret material or underlying library error text crosses the ABI.
/// The caller must release the pointer exactly once using `hatoba_string_free`.
#[unsafe(no_mangle)]
pub extern "C" fn hatoba_native_probe() -> *mut c_char {
    let result = std::panic::catch_unwind(|| serde_json::to_string(&probe()));
    let json = match result {
        Ok(Ok(json)) => json,
        _ => r#"{"error":"native_probe_failed"}"#.into(),
    };
    CString::new(json)
        .expect("JSON contains no literal NUL")
        .into_raw()
}

/// # Safety
/// `value` must be null or a pointer returned by this library, not already freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn hatoba_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_crypto_storage_ssh_and_runtime_probe_passes() {
        let report = probe();
        assert!(report.passed(), "{report:?}");
    }

    #[test]
    fn c_abi_owns_and_releases_json() {
        let value = hatoba_native_probe();
        let json = unsafe { std::ffi::CStr::from_ptr(value) }.to_str().unwrap();
        let report: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(report["host_protocol"], HOST_PROTOCOL);
        unsafe { hatoba_string_free(value) };
        unsafe { hatoba_string_free(std::ptr::null_mut()) };
    }
}
