// Composes the archive provider: where it finds archive files, the notice of a slow listing, and the password of an encrypted archive
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// `waypoint-provider-archive` serves `archive:` locations from zip, tar and 7z files (D152, D154). It
// reads the archive file through whatever provider holds it, so this is where it is given the local
// disk and the registry of the server providers (the vfs plugin builds that registry from the
// providers listed here, the archive provider among them, so it is reached through a cell filled once
// the plugin is set up). The same provider, typed, also lists archives for Extract and makes them
// for Compress (A92), so the operations plugin is handed it by `ops::compose`. A slow listing and an
// encrypted archive are what the person must be told or asked: the first is an event the page shows
// in the tab that is waiting, the second a command that gives the provider the password (it never
// goes in a location or a log). Built behind the `archive` feature (A85).

#[cfg(feature = "archive")]
pub use enabled::*;

#[cfg(not(feature = "archive"))]
pub use disabled::*;

/// The event a slow listing is announced with.
pub const NOTICE_EVENT: &str = "archive-notice";

#[cfg(feature = "archive")]
mod enabled {
    use std::sync::{Arc, OnceLock};

    use serde::Serialize;
    use tauri::{AppHandle, Emitter, Manager, State, Wry};
    use waypoint_path::VfsPath;
    use waypoint_protocol::{Location, VfsError};
    use waypoint_provider_archive::{
        ArchiveFormat, ArchiveNotice, ArchiveOptions, ArchiveProvider, ContainerSource,
    };
    use waypoint_vfs::{LocalProvider, Provider, ProviderRegistry, Secret};

    use super::NOTICE_EVENT;

    /// What the page hears when a listing will take a while: the archive file's location, its size
    /// in bytes and its format as people name it.
    #[derive(Debug, Clone, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "camelCase")]
    pub struct SlowListing {
        pub container: Location,
        pub bytes: u64,
        pub format: String,
    }

    /// The archive provider and what it needs once the app exists.
    #[derive(Clone)]
    pub struct Archives {
        pub provider: Arc<ArchiveProvider>,
        registry: Arc<OnceLock<Arc<ProviderRegistry>>>,
    }

    /// Finds the provider of an archive file: the disk for a local file, the server providers for a
    /// remote one.
    struct Containers {
        local: Arc<dyn Provider>,
        registry: Arc<OnceLock<Arc<ProviderRegistry>>>,
    }

    impl ContainerSource for Containers {
        fn provider_for(&self, container: &VfsPath) -> Result<Arc<dyn Provider>, VfsError> {
            match container {
                VfsPath::File(_) => Ok(self.local.clone()),
                other => self
                    .registry
                    .get()
                    .ok_or_else(|| VfsError::Unsupported {
                        what: other.scheme().to_owned(),
                    })?
                    .for_path(other),
            }
        }
    }

    impl Archives {
        pub fn new() -> Self {
            let registry: Arc<OnceLock<Arc<ProviderRegistry>>> = Arc::new(OnceLock::new());
            let containers = Containers {
                local: Arc::new(LocalProvider::new()),
                registry: registry.clone(),
            };
            Self {
                provider: ArchiveProvider::new(Arc::new(containers), ArchiveOptions::default()),
                registry,
            }
        }

        /// Gives the provider the registry it reads server-held archives through, and sends its
        /// notices to every window.
        pub fn wire(&self, app: &AppHandle<Wry>) {
            if let Some(vfs) = app.try_state::<tauri_plugin_waypoint_vfs::Vfs>() {
                let _ = self.registry.set(vfs.remote().clone());
            }
            let app = app.clone();
            self.provider.set_notice_sink(Some(Arc::new(move |notice| {
                let ArchiveNotice::SlowListing {
                    container,
                    format,
                    bytes,
                } = notice;
                let event = SlowListing {
                    container,
                    bytes,
                    format: label(format).to_owned(),
                };
                if let Err(e) = app.emit(NOTICE_EVENT, event) {
                    log::warn!("could not announce a slow archive listing: {e}");
                }
            })));
        }
    }

    impl Default for Archives {
        fn default() -> Self {
            Self::new()
        }
    }

    fn label(format: ArchiveFormat) -> &'static str {
        format.label()
    }

    /// The archive file a location is in or is: the location of an `archive:` path's container, or
    /// a file's own.
    pub fn container_of(location: &Location) -> Result<VfsPath, VfsError> {
        let invalid = || VfsError::InvalidLocation {
            input: location.uri.clone(),
        };
        match VfsPath::from_location(location).map_err(|_| invalid())? {
            VfsPath::Archive(archive) => Ok(archive.container().clone()),
            file @ VfsPath::File(_) => Ok(file),
            _ => Err(invalid()),
        }
    }

    /// Gives the provider the password of the archive file `location` is, or is in. The tab asks
    /// again afterwards; a wrong password is `AuthFailed` the next time the archive is read. The
    /// password is kept in memory only.
    #[tauri::command]
    pub async fn unlock_archive(
        archives: State<'_, Archives>,
        location: Location,
        passphrase: String,
    ) -> Result<(), VfsError> {
        let container = container_of(&location)?;
        archives
            .provider
            .unlock(&container, Secret::from(passphrase));
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// A zip holding one stored file.
        fn stored_zip(name: &str, data: &[u8]) -> Vec<u8> {
            let mut crc = !0u32;
            for &byte in data {
                crc ^= u32::from(byte);
                for _ in 0..8 {
                    crc = if crc & 1 == 1 {
                        (crc >> 1) ^ 0xedb8_8320
                    } else {
                        crc >> 1
                    };
                }
            }
            let crc = !crc;
            let size = data.len() as u32;
            let mut out = Vec::new();
            out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
            out.extend_from_slice(&[20, 0, 0, 8, 0, 0, 0, 0, 0x21, 0x58]);
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);
            let mut central = Vec::new();
            central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
            central.extend_from_slice(&[20, 3, 20, 0, 0, 8, 0, 0, 0, 0, 0x21, 0x58]);
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&(name.len() as u16).to_le_bytes());
            central.extend_from_slice(&[0u8; 8]);
            central.extend_from_slice(&(0o100_644u32 << 16).to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(name.as_bytes());
            let at = out.len() as u32;
            out.extend_from_slice(&central);
            out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
            out.extend_from_slice(&[0, 0, 0, 0, 1, 0, 1, 0]);
            out.extend_from_slice(&(central.len() as u32).to_le_bytes());
            out.extend_from_slice(&at.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out
        }

        fn location(uri: &str) -> Location {
            Location::new(uri, uri)
        }

        #[test]
        fn the_container_of_an_archive_location_is_its_file() {
            let inside = location("archive:file:///home/me/a.zip!/docs");
            assert_eq!(
                container_of(&inside).unwrap().to_uri(),
                "file:///home/me/a.zip"
            );
            let file = location("file:///home/me/a.zip");
            assert_eq!(
                container_of(&file).unwrap().to_uri(),
                "file:///home/me/a.zip"
            );
            // An archive inside an archive: the file is the one that holds it.
            let nested = location("archive:archive:file:///a.zip!/b.tar!/x");
            assert_eq!(
                container_of(&nested).unwrap().to_uri(),
                "archive:file:///a.zip!/b.tar"
            );
            assert!(container_of(&location("trash:///")).is_err());
        }

        #[test]
        fn a_local_archive_lists_through_the_composed_provider() {
            let dir = tempfile::tempdir().unwrap();
            let zip = dir.path().join("pack.zip");
            std::fs::write(&zip, stored_zip("hello.txt", b"hello")).unwrap();
            let archive = VfsPath::from_uri(&format!(
                "archive:{}!/",
                VfsPath::File(waypoint_path::FilePath::from_path(&zip).unwrap()).to_uri()
            ))
            .unwrap();
            let archives = Archives::new();
            let names: Vec<_> = archives
                .provider
                .list(&archive, &waypoint_vfs::CancelToken::new(), 0, &mut |_| {})
                .unwrap()
                .into_iter()
                .map(|e| e.name)
                .collect();
            assert_eq!(names, ["hello.txt"]);
        }

        #[test]
        fn the_provider_serves_the_archive_scheme_and_a_local_file_resolves() {
            let archives = Archives::new();
            assert_eq!(archives.provider.scheme(), "archive");
            let containers = Containers {
                local: Arc::new(LocalProvider::new()),
                registry: archives.registry.clone(),
            };
            let file = VfsPath::from_uri("file:///a.zip").unwrap();
            assert!(containers.provider_for(&file).is_ok());
            // A server file before the vfs plugin is set up has no provider yet, and says so.
            let remote = VfsPath::from_uri("sftp://me@nas.lan/a.zip").unwrap();
            assert!(matches!(
                containers.provider_for(&remote),
                Err(VfsError::Unsupported { .. })
            ));
        }
    }
}

#[cfg(not(feature = "archive"))]
mod disabled {
    use tauri::State;
    use waypoint_protocol::{Location, VfsError};

    /// Archives left out of this build.
    #[derive(Clone, Default)]
    pub struct Archives;

    impl Archives {
        pub fn new() -> Self {
            Self
        }

        pub fn wire(&self, _app: &tauri::AppHandle<tauri::Wry>) {}
    }

    #[tauri::command]
    pub async fn unlock_archive(
        _archives: State<'_, Archives>,
        _location: Location,
        _passphrase: String,
    ) -> Result<(), VfsError> {
        Err(VfsError::Unsupported {
            what: "archives in this build".to_owned(),
        })
    }
}
