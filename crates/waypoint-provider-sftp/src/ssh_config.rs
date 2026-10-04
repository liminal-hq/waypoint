// Where a location's host name really leads: `~/.ssh/config`'s host name, user, port, key files
// and jump hosts, behind a trait so tests and the app choose the source.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::PathBuf;

use openssh_config::{HostConfig, Jump, SshConfig};
use waypoint_path::{Authority, Host, RemotePath, RemoteScheme};

use crate::options::local_user;
use crate::session::{host_name, Target};

/// Where the options of a host name come from (`openssh_config::SshConfig` for `~/.ssh/config`).
pub trait SshConfigSource: Send + Sync {
    /// The options that apply to `alias`, the host name as a location writes it.
    fn host(&self, alias: &str) -> HostConfig;
}

impl SshConfigSource for SshConfig {
    fn host(&self, alias: &str) -> HostConfig {
        SshConfig::host(self, alias)
    }
}

/// Resolves a location into the hops to reach it. What the location says wins over the config,
/// as the command line does over `ssh_config`; key files are the configuration's when it names
/// some (`explicit` overrides both), the defaults otherwise.
pub(crate) fn resolve(
    path: &RemotePath,
    source: Option<&dyn SshConfigSource>,
    explicit: Option<&[PathBuf]>,
    defaults: &dyn Fn() -> Vec<PathBuf>,
) -> Target {
    let authority = path.authority();
    let alias = host_name(&authority.host);
    let config = source.map(|source| source.host(&alias)).unwrap_or_default();
    let files = |config: &HostConfig| match explicit {
        Some(files) => files.to_vec(),
        None if !config.identity_files.is_empty() => config.identity_files.clone(),
        None => defaults(),
    };
    let jumps = config
        .proxy_jump
        .iter()
        .map(|jump| jump_target(jump, source, &files))
        .collect();
    Target {
        key: path.connection_key(),
        host: config.host_name.clone().unwrap_or(alias),
        port: authority.port.or(config.port).unwrap_or(22),
        user: authority
            .user
            .clone()
            .or(config.user.clone())
            .unwrap_or_else(local_user),
        identity_files: files(&config),
        jumps,
    }
}

/// One jump host, with its own options from the configuration (but not its own `ProxyJump`, so
/// a chain cannot loop).
fn jump_target(
    jump: &Jump,
    source: Option<&dyn SshConfigSource>,
    files: &dyn Fn(&HostConfig) -> Vec<PathBuf>,
) -> Target {
    let config = source
        .map(|source| source.host(&jump.host))
        .unwrap_or_default();
    let user = jump
        .user
        .clone()
        .or(config.user.clone())
        .unwrap_or_else(local_user);
    let port = jump.port.or(config.port).unwrap_or(22);
    let key = RemotePath::root(
        RemoteScheme::Sftp,
        Authority {
            user: Some(user.clone()),
            host: Host::Name(jump.host.to_lowercase()),
            port: (port != 22).then_some(port),
        },
    )
    .connection_key();
    Target {
        key,
        host: config
            .host_name
            .clone()
            .unwrap_or_else(|| jump.host.clone()),
        port,
        user,
        identity_files: files(&config),
        jumps: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use waypoint_path::VfsPath;

    use super::*;

    fn remote(uri: &str) -> RemotePath {
        let VfsPath::Remote(path) = VfsPath::from_uri(uri).unwrap() else {
            panic!("{uri} is remote")
        };
        path
    }

    #[test]
    fn an_alias_takes_its_address_user_port_keys_and_jumps_from_the_config() {
        let config = SshConfig::parse(
            "Host nas\n  HostName 10.0.0.5\n  User media\n  Port 2222\n  IdentityFile /k/nas\n  ProxyJump me@gw:2200,inner\n\
             Host gw\n  HostName gw.example.com\n  IdentityFile /k/gw\n\
             Host inner\n  User admin\n",
            Path::new("/"),
        );
        let defaults = || vec![PathBuf::from("/k/default")];
        let target = resolve(&remote("sftp://nas/srv"), Some(&config), None, &defaults);
        assert_eq!(target.host, "10.0.0.5");
        assert_eq!(target.port, 2222);
        assert_eq!(target.user, "media");
        assert_eq!(target.key.as_str(), "sftp://nas");
        assert_eq!(target.identity_files, [PathBuf::from("/k/nas")]);
        assert_eq!(target.jumps.len(), 2);
        assert_eq!(target.jumps[0].host, "gw.example.com");
        assert_eq!(target.jumps[0].user, "me");
        assert_eq!(target.jumps[0].port, 2200);
        assert_eq!(target.jumps[0].identity_files, [PathBuf::from("/k/gw")]);
        assert_eq!(target.jumps[0].key.as_str(), "sftp://me@gw:2200");
        assert_eq!(target.jumps[1].user, "admin");
        assert_eq!(
            target.jumps[1].identity_files,
            [PathBuf::from("/k/default")]
        );
        // The location's own user and port win.
        let own = resolve(
            &remote("sftp://root@nas:22/"),
            Some(&config),
            None,
            &defaults,
        );
        assert_eq!(own.user, "root");
        assert_eq!(own.port, 2222, "port 22 is the default and is not written");
        let own = resolve(
            &remote("sftp://root@nas:2000/"),
            Some(&config),
            None,
            &defaults,
        );
        assert_eq!(own.port, 2000);
        // Without a config the location is taken as written.
        let plain = resolve(&remote("sftp://nas/"), None, None, &defaults);
        assert_eq!(plain.host, "nas");
        assert_eq!(plain.identity_files, [PathBuf::from("/k/default")]);
        assert!(plain.jumps.is_empty());
    }
}
