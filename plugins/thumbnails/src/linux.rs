// The Linux implementation: the freedesktop.org thumbnail cache, the built-in generator and external thumbnailers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tauri::{AppHandle, Runtime};

use crate::builtin::{self, GenError, Rendered};
use crate::cache::{file_uri, Meta, Store, UriFn};
use crate::engine::{Limits, Outcome, Processor};
use crate::external::{self, RunError};
use crate::memcache::MemCache;
use crate::mime::MimeDb;
use crate::models::{
    Config, FeatureStatus, Flavour, PluginStatus, ReasonKind, SkipWhy, ThumbRequest,
    FEATURE_BUILTIN, FEATURE_CACHE, FEATURE_EXTERNAL, FEATURE_SHELL,
};
use crate::pipeline::{self, Start};
use crate::thumbnailer::{self, ExecValues, Thumbnailer};

/// The most output an external thumbnailer may leave, which is far more than a thumbnail needs.
const MAX_EXTERNAL_OUTPUT: u64 = 64 * 1024 * 1024;

/// Everything about the system the plugin reads, injected so a test works in a temporary directory and never touches the real home.
#[derive(Clone)]
pub struct Env {
    /// The folder that holds `normal`, `large`, `x-large`, `xx-large` and `fail`: `$XDG_CACHE_HOME/thumbnails`.
    pub cache_root: PathBuf,
    /// Where `thumbnailers/*.thumbnailer` and `mime/globs2` are looked for, most important first.
    pub data_dirs: Vec<PathBuf>,
    /// The folders a bare `TryExec` name is looked for in.
    pub path_dirs: Vec<PathBuf>,
    /// Names a file in the cache.
    pub uri_of: UriFn,
}

impl Env {
    /// The real system: `XDG_CACHE_HOME` (or `~/.cache`), the data directories and `PATH` of this process.
    pub fn system<R: Runtime>(_app: &AppHandle<R>) -> Env {
        Env::from_vars(|name| std::env::var(name).ok())
    }

    /// Builds the environment from variables (`XDG_CACHE_HOME`, `XDG_DATA_HOME`, `XDG_DATA_DIRS`, `HOME`, `PATH`), read through `get`.
    pub fn from_vars(get: impl Fn(&str) -> Option<String>) -> Env {
        let non_empty = |name: &str| get(name).filter(|value| !value.is_empty());
        let home = non_empty("HOME").map(PathBuf::from);
        let cache_home = non_empty("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| home.as_ref().map(|home| home.join(".cache")))
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let data_home = non_empty("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| home.as_ref().map(|home| home.join(".local/share")));
        let system_dirs =
            non_empty("XDG_DATA_DIRS").unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
        let data_dirs = data_home
            .into_iter()
            .chain(system_dirs.split(':').map(PathBuf::from))
            .filter(|path| path.is_absolute())
            .collect();
        let path_dirs = non_empty("PATH")
            .map(|path| std::env::split_paths(&path).collect())
            .unwrap_or_default();
        Env {
            cache_root: cache_home.join("thumbnails"),
            data_dirs,
            path_dirs,
            uri_of: Arc::new(file_uri),
        }
    }
}

pub struct Platform {
    store: Arc<Store>,
    processor: Arc<FreedesktopProcessor>,
    status: PluginStatus,
}

impl Platform {
    pub fn new(env: Env, config: &Config, mem: Arc<MemCache>, limits: Arc<Limits>) -> Self {
        let store = Arc::new(Store::new(
            env.cache_root.clone(),
            &config.app_name,
            &config.app_version,
            Arc::clone(&env.uri_of),
        ));
        let thumbnailers = thumbnailer::discover(&env.data_dirs, &env.path_dirs);
        let status = status(&env, thumbnailers.len());
        let processor = Arc::new(FreedesktopProcessor {
            store: Arc::clone(&store),
            mem,
            limits,
            mime: MimeDb::load(&env.data_dirs),
            thumbnailers,
            path_dirs: env.path_dirs.clone(),
        });
        Platform {
            store,
            processor,
            status,
        }
    }

    pub fn store(&self) -> Arc<Store> {
        Arc::clone(&self.store)
    }

    pub fn processor(&self) -> Arc<dyn Processor> {
        Arc::clone(&self.processor) as Arc<dyn Processor>
    }

    pub fn status(&self) -> PluginStatus {
        self.status.clone()
    }
}

fn status(env: &Env, thumbnailers: usize) -> PluginStatus {
    let cache = match fs::create_dir_all(&env.cache_root) {
        Ok(()) => FeatureStatus::available(FEATURE_CACHE),
        Err(error) => FeatureStatus::unavailable(
            FEATURE_CACHE,
            ReasonKind::NoCacheDirectory,
            format!(
                "the thumbnail cache {} cannot be used: {error}",
                env.cache_root.display()
            ),
        ),
    };
    let external = if thumbnailers == 0 {
        FeatureStatus::unavailable(
            FEATURE_EXTERNAL,
            ReasonKind::NoThumbnailers,
            "no *.thumbnailer file is installed (PDF, video and font thumbnails need one)",
        )
    } else {
        FeatureStatus::available_with_count(FEATURE_EXTERNAL, thumbnailers as u32)
    };
    PluginStatus::build(
        Flavour::Freedesktop,
        vec![
            cache,
            FeatureStatus::available(FEATURE_BUILTIN),
            external,
            FeatureStatus::unavailable(
                FEATURE_SHELL,
                ReasonKind::OtherPlatform,
                "the Windows shell makes thumbnails only on Windows",
            ),
        ],
    )
}

/// Makes thumbnails the freedesktop way: the shared cache first, then the built-in decoders, then an installed thumbnailer.
pub struct FreedesktopProcessor {
    store: Arc<Store>,
    mem: Arc<MemCache>,
    limits: Arc<Limits>,
    mime: MimeDb,
    thumbnailers: Vec<Thumbnailer>,
    path_dirs: Vec<PathBuf>,
}

/// True for an absolute path on this machine's file system; a remote location (anything with a scheme, such as `sftp://host/x`) and a relative path are not.
pub fn is_local(path: &str) -> bool {
    path.starts_with('/') && !path.contains("://")
}

impl Processor for FreedesktopProcessor {
    fn process(&self, request: &ThumbRequest, cancel: &AtomicBool) -> Outcome {
        if !is_local(&request.path) {
            return Outcome::Skipped {
                why: SkipWhy::Remote,
            };
        }
        let path = Path::new(&request.path);
        let (key, uri, secs) = match pipeline::begin(&self.store, &self.mem, request) {
            Start::Done(outcome) => return outcome,
            Start::Make { key, uri, secs } => (key, uri, secs),
        };
        let file_size = match fs::metadata(path) {
            Ok(meta) if meta.is_file() => meta.len(),
            Ok(_) => {
                return Outcome::Skipped {
                    why: SkipWhy::NoGenerator,
                }
            }
            Err(error) => {
                return Outcome::Failed {
                    reason: error.to_string(),
                }
            }
        };
        let meta = Meta {
            uri: uri.clone(),
            mtime_secs: secs,
            file_size: Some(file_size),
        };
        match self.generate(request, path, &uri, cancel) {
            Generated::Made(rendered) => {
                pipeline::finish(&self.store, &self.mem, &key, &meta, &rendered)
            }
            Generated::Skipped(why) => Outcome::Skipped { why },
            Generated::Cancelled => Outcome::Cancelled,
            // Remembered, so a broken file is not decoded again at every scroll.
            Generated::Failed(reason) => pipeline::fail(&self.store, &key, &meta, reason),
        }
    }
}

enum Generated {
    Made(Rendered),
    Skipped(SkipWhy),
    Failed(String),
    Cancelled,
}

impl FreedesktopProcessor {
    fn generate(
        &self,
        request: &ThumbRequest,
        path: &Path,
        uri: &str,
        cancel: &AtomicBool,
    ) -> Generated {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        let mime = self.mime.mime_of(&name);
        let side = request.size.pixels();
        let mut failure: Option<String> = None;
        // An image the plugin decodes itself, or a file whose name says nothing (its content decides).
        if mime.is_none_or(builtin::handles_mime) {
            match builtin::generate(path, side, self.limits.max_file_bytes()) {
                Ok(rendered) => return Generated::Made(rendered),
                Err(GenError::TooLarge) => return Generated::Skipped(SkipWhy::TooLarge),
                Err(GenError::Unsupported) => {}
                Err(GenError::Failed(reason)) => failure = Some(reason),
            }
        }
        if let Some(mime) = mime {
            for thumbnailer in self.thumbnailers.iter().filter(|t| t.handles(mime)) {
                match self.run_external(thumbnailer, request, path, uri, cancel) {
                    Ok(rendered) => return Generated::Made(rendered),
                    Err(RunError::Cancelled) => return Generated::Cancelled,
                    Err(error) => failure = Some(format!("{}: {error}", thumbnailer.id)),
                }
            }
        }
        match failure {
            Some(reason) => Generated::Failed(reason),
            None => Generated::Skipped(SkipWhy::NoGenerator),
        }
    }

    /// Runs one external thumbnailer into a scratch file in the cache and reads what it made.
    fn run_external(
        &self,
        thumbnailer: &Thumbnailer,
        request: &ThumbRequest,
        path: &Path,
        uri: &str,
        cancel: &AtomicBool,
    ) -> Result<Rendered, RunError> {
        let side = request.size.pixels();
        let output = self
            .store
            .scratch_path(request.size)
            .map_err(|error| RunError::Spawn(error.to_string()))?;
        let result = (|| {
            let argv = thumbnailer
                .command(&ExecValues {
                    input: path,
                    uri,
                    output: &output,
                    size: side,
                })
                .ok_or_else(|| RunError::Spawn("the thumbnailer has no command".to_string()))?;
            let mut argv = argv;
            argv[0] = thumbnailer::resolve_program(&argv[0], &self.path_dirs)
                .ok_or_else(|| RunError::Spawn(format!("{} was not found", argv[0])))?
                .to_string_lossy()
                .into_owned();
            external::run(&argv, self.limits.external_timeout(), cancel)?;
            let len = fs::metadata(&output)
                .map_err(|_| RunError::Spawn("the thumbnailer made no file".to_string()))?
                .len();
            if len > MAX_EXTERNAL_OUTPUT {
                return Err(RunError::Spawn(
                    "the thumbnailer's output is too large".to_string(),
                ));
            }
            let bytes = fs::read(&output).map_err(|error| RunError::Spawn(error.to_string()))?;
            builtin::generate_from_bytes(&bytes, side)
                .map_err(|error| RunError::Spawn(format!("the output is not an image ({error:?})")))
        })();
        let _ = fs::remove_file(&output);
        result
    }
}

#[cfg(test)]
mod tests;
