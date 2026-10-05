// The AWS environment variables and shared files, read as one optional source of credentials.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Only the plain forms are read: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`,
//! `AWS_SESSION_TOKEN` and `AWS_REGION` (or `AWS_DEFAULT_REGION`), and a profile (`AWS_PROFILE`,
//! else `default`) of `~/.aws/credentials` and `~/.aws/config` (or the files
//! `AWS_SHARED_CREDENTIALS_FILE` and `AWS_CONFIG_FILE` name). SSO, `credential_process`, role
//! assumption and instance or container metadata are not read: a login that needs them is answered
//! in the Connect dialog instead. The provider offers these credentials to AWS endpoints only, never
//! to another service, so a MinIO server cannot be handed an AWS key by accident.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use waypoint_vfs::Secret;

/// What the environment and the shared files hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbientCredentials {
    pub key_id: String,
    pub secret: Secret,
    pub session_token: Option<Secret>,
    pub region: Option<String>,
}

/// Reads the ambient credentials from the process environment and the user's AWS files.
pub fn read() -> Option<AmbientCredentials> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    read_from(&|name| std::env::var(name).ok(), home.as_deref(), &|path| {
        std::fs::read_to_string(path).ok()
    })
}

/// `read` with the environment, the home folder and the files given, so a test controls them.
pub(crate) fn read_from(
    env: &dyn Fn(&str) -> Option<String>,
    home: Option<&Path>,
    file: &dyn Fn(&Path) -> Option<String>,
) -> Option<AmbientCredentials> {
    let non_empty = |name: &str| env(name).filter(|value| !value.is_empty());
    let profile = non_empty("AWS_PROFILE").unwrap_or_else(|| "default".to_owned());
    let region_from_env = non_empty("AWS_REGION").or_else(|| non_empty("AWS_DEFAULT_REGION"));
    let config_path = non_empty("AWS_CONFIG_FILE")
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join(".aws").join("config")));
    let region = region_from_env.or_else(|| {
        let text = file(config_path.as_deref()?)?;
        let sections = parse_ini(&text);
        let section = if profile == "default" {
            "default".to_owned()
        } else {
            format!("profile {profile}")
        };
        sections.get(&section)?.get("region").cloned()
    });
    if let (Some(key_id), Some(secret)) = (
        non_empty("AWS_ACCESS_KEY_ID"),
        non_empty("AWS_SECRET_ACCESS_KEY"),
    ) {
        return Some(AmbientCredentials {
            key_id,
            secret: Secret::from(secret),
            session_token: non_empty("AWS_SESSION_TOKEN").map(Secret::from),
            region,
        });
    }
    let credentials_path = non_empty("AWS_SHARED_CREDENTIALS_FILE")
        .map(PathBuf::from)
        .or_else(|| home.map(|home| home.join(".aws").join("credentials")))?;
    let sections = parse_ini(&file(&credentials_path)?);
    let section = sections.get(&profile)?;
    Some(AmbientCredentials {
        key_id: section.get("aws_access_key_id")?.clone(),
        secret: Secret::from(section.get("aws_secret_access_key")?.clone()),
        session_token: section.get("aws_session_token").cloned().map(Secret::from),
        region,
    })
}

type Sections = HashMap<String, HashMap<String, String>>;

/// The `[section]` and `key = value` lines of an AWS file; `#` and `;` start a comment line.
fn parse_ini(text: &str) -> Sections {
    let mut sections = Sections::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            let name = name.trim().to_owned();
            sections.entry(name.clone()).or_default();
            current = Some(name);
        } else if let (Some(section), Some((key, value))) = (&current, line.split_once('=')) {
            sections
                .entry(section.clone())
                .or_default()
                .insert(key.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREDENTIALS: &str = "# a comment\n[default]\naws_access_key_id = AKIADEFAULT\naws_secret_access_key = s3cret\n\n[work]\naws_access_key_id=AKIAWORK\naws_secret_access_key=w0rk\naws_session_token=tok\n";
    const CONFIG: &str = "[default]\nregion = ca-central-1\n\n[profile work]\nregion = eu-west-2\n";

    fn run(env: &[(&str, &str)]) -> Option<AmbientCredentials> {
        let env: HashMap<String, String> = env
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        read_from(
            &|name| env.get(name).cloned(),
            Some(Path::new("/home/me")),
            &|path| match path.to_str()? {
                "/home/me/.aws/credentials" => Some(CREDENTIALS.to_owned()),
                "/home/me/.aws/config" => Some(CONFIG.to_owned()),
                _ => None,
            },
        )
    }

    #[test]
    fn the_environment_wins_and_a_profile_is_the_fallback() {
        let from_env = run(&[
            ("AWS_ACCESS_KEY_ID", "AKIAENV"),
            ("AWS_SECRET_ACCESS_KEY", "envsecret"),
            ("AWS_SESSION_TOKEN", "envtoken"),
            ("AWS_REGION", "us-west-2"),
        ])
        .unwrap();
        assert_eq!(from_env.key_id, "AKIAENV");
        assert_eq!(from_env.secret.expose_str(), Some("envsecret"));
        assert_eq!(
            from_env.session_token.unwrap().expose_str(),
            Some("envtoken")
        );
        assert_eq!(from_env.region.as_deref(), Some("us-west-2"));

        let default = run(&[]).unwrap();
        assert_eq!(default.key_id, "AKIADEFAULT");
        assert_eq!(default.region.as_deref(), Some("ca-central-1"));
        assert!(default.session_token.is_none());

        let work = run(&[("AWS_PROFILE", "work")]).unwrap();
        assert_eq!(work.key_id, "AKIAWORK");
        assert_eq!(work.session_token.unwrap().expose_str(), Some("tok"));
        assert_eq!(work.region.as_deref(), Some("eu-west-2"));
    }

    #[test]
    fn nothing_found_is_none_and_the_secret_never_prints() {
        assert!(run(&[("AWS_PROFILE", "missing")]).is_none());
        let none = read_from(&|_| None, None, &|_| None);
        assert!(none.is_none());
        let found = run(&[]).unwrap();
        assert!(!format!("{found:?}").contains("s3cret"));
    }
}
