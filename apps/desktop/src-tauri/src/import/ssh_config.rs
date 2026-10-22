//! `~/.ssh/config` (`%USERPROFILE%\.ssh\config` on Windows), SSH-11.

use hatoba_ssh::SshConfigHost;
use std::path::Path;

use super::{Entry, Jump, Source};
use crate::error::{AppError, AppResult};

pub(super) fn read(selected: Option<&Path>) -> AppResult<Source> {
    let home = if selected.is_none() {
        dirs::home_dir()
    } else {
        None
    };
    let path = match selected {
        Some(path) => path.to_owned(),
        None => home
            .as_ref()
            .ok_or_else(|| AppError::not_found("home directory"))?
            .join(".ssh")
            .join("config"),
    };
    let text = match super::text::read_file(&path) {
        Ok(bytes) => super::text::decode(&bytes),
        Err(error) if selected.is_none() && !path.exists() => {
            let _ = error;
            String::new()
        }
        Err(error) => return Err(error),
    };
    let home = home.as_deref().map(|home| home.to_string_lossy());
    Ok(Source {
        location: Some(path.to_string_lossy().into_owned()),
        encrypted: false,
        entries: hatoba_ssh::config::parse_ssh_config_with_home(&text, home.as_deref())
            .into_iter()
            .map(entry)
            .collect(),
        ..Source::default()
    })
}

fn entry(host: SshConfigHost) -> Entry {
    Entry {
        id: host.alias.clone(),
        address: host.hostname.unwrap_or_else(|| host.alias.clone()),
        name: host.alias,
        port: host.port.unwrap_or(22),
        username: host.user,
        identity_files: host.identity_files,
        jump: host.proxy_jump.map(Jump::Named),
        skipped_proxy: host.proxy_command.map(|c| format!("ProxyCommand {c}")),
        set_env: host.set_env,
        ..Entry::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_config_uses_only_the_selected_file_and_keeps_foreign_identity_paths() {
        let path =
            std::env::temp_dir().join(format!("hatoba-selected-config-{}", uuid::Uuid::new_v4()));
        std::fs::write(&path, "Host generated\n HostName 127.0.0.1\n User fixture\n Port 22222\n IdentityFile ~/.ssh/foreign-key\n").unwrap();
        let source = read(Some(&path)).unwrap();
        assert_eq!(source.entries.len(), 1);
        assert_eq!(source.entries[0].address, "127.0.0.1");
        assert_eq!(source.entries[0].port, 22222);
        assert_eq!(source.entries[0].identity_files, ["~/.ssh/foreign-key"]);
        std::fs::remove_file(path).unwrap();
    }
}
