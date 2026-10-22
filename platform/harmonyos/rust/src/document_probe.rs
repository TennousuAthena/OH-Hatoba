//! Generated-data document checks without opening the user picker or reading user files.
use crate::files::Grants;
use hatoba_runtime::{commands::skills::read_named_source, dto::ImportSource, error::AppResult};
use std::{
    fs::{self, File},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
};

struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub async fn run(cache: &Path) -> AppResult<bool> {
    let area = cache.join(format!("hatoba-document-probe-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&area)?;
    let _cleanup = Cleanup(area.clone());
    let grants = Grants::new(&area)?;
    let config = area.join("selected-config");
    fs::write(
        &config,
        "Host generated\n HostName 127.0.0.1\n User fixture\n Port 22222\n IdentityFile ~/.ssh/foreign-key\n",
    )?;
    let file = File::open(&config)?;
    let handle = unsafe { grants.register(file.as_raw_fd(), "config", false) }?;
    drop(file);
    let resolved = grants.resolve(&handle, false)?;
    let imported =
        hatoba_runtime::import::read(ImportSource::SshConfig, Some(Path::new(&resolved)))?;
    let config_ok = imported.entries.len() == 1
        && imported.entries[0].port == 22222
        && imported.entries[0].identity_files == ["~/.ssh/foreign-key"];
    let mut forged = serde_json::json!({"source":"ssh_config", "path":handle, "ids":["generated"], "keyFiles":[config.to_string_lossy()]});
    let denied = grants.prepare("hosts_import", &mut forged).is_err();
    grants.release(&resolved);
    let revoked = grants.resolve(&handle, false).is_err() && config.exists();

    let zip = area.join("selected-archive");
    fs::write(&zip, include_bytes!("fixtures/generated-skill.zip"))?;
    let file = File::open(&zip)?;
    let handle = unsafe { grants.register(file.as_raw_fd(), "generated.zip", false) }?;
    drop(file);
    let resolved = grants.resolve(&handle, false)?;
    let name = grants.input_name(&resolved)?;
    let zip_ok = read_named_source(Path::new(&resolved), Path::new(&name))?
        .package
        .is_some_and(|package| {
            package.name == "diagnostic-fixture" && package.body.contains("中文 🌊")
        });
    grants.release(&handle);

    let folder = area.join("selected-folder");
    fs::create_dir(&folder)?;
    fs::write(
        folder.join("SKILL.md"),
        "---\nname: diagnostic-fixture\ndescription: Generated folder test only\n---\nGenerated 中文 🌊\n",
    )?;
    let file = File::open(&folder)?;
    let handle = unsafe { grants.register(file.as_raw_fd(), "generated-folder", false) }?;
    drop(file);
    #[cfg(not(target_os = "macos"))]
    let resolved = grants.resolve(&handle, false)?;
    // macOS /dev/fd cannot traverse directories; OHOS must exercise its real /proc FD path.
    #[cfg(target_os = "macos")]
    let folder_source = folder.as_path();
    #[cfg(not(target_os = "macos"))]
    let folder_source = Path::new(&resolved);
    let folder_ok = read_named_source(folder_source, Path::new("generated-folder"))?
        .package
        .is_some_and(|package| package.name == "diagnostic-fixture");
    grants.release(&handle);

    let output = area.join("selected-output");
    let file = File::create(&output)?;
    let handle = unsafe { grants.register(file.as_raw_fd(), "generated.txt", true) }?;
    drop(file);
    let staged = grants.resolve(&handle, true)?;
    grants.write_text(&staged, "Generated 中文 🌊").await?;
    let output_ok = fs::read_to_string(output)? == "Generated 中文 🌊";
    grants.release(&handle);

    Ok(config_ok
        && denied
        && revoked
        && zip_ok
        && folder_ok
        && output_ok
        && !Path::new(&staged).exists())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn generated_documents_preserve_names_and_scope_their_grants() {
        assert!(super::run(&std::env::temp_dir()).await.unwrap());
    }
}
