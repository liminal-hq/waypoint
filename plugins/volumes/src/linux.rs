// The Linux backend: UDisks2 over the system bus, and the kernel's mount table for what UDisks2 does not manage
//
// Where there is no UDisks2 to talk to (no system bus, a Flatpak without access to it, a minimal system) the plugin lists from the mount table alone and reports the actions unavailable, with the reason.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::task::AbortHandle;
use zbus::export::futures_util::StreamExt;
use zbus::zvariant::{OwnedObjectPath, Value};
use zbus::{fdo, Connection, DBusError, MatchRule, MessageStream, MessageType, Proxy};

use crate::backend::{Backend, BoxFuture, Notify};
use crate::error::{Result, VolumesError};
use crate::holders::find_holders;
use crate::models::{
    FeatureStatus, Flavour, Passphrase, PluginStatus, Reason, Volume, FEATURE_EJECT, FEATURE_LIST,
    FEATURE_MOUNT, FEATURE_UNLOCK, FEATURE_UNMOUNT, FEATURE_WATCH,
};
use crate::{mountinfo, udisks};

const SERVICE: &str = "org.freedesktop.UDisks2";
const ROOT: &str = "/org/freedesktop/UDisks2";
const MOUNT_TABLE: &str = "/proc/self/mountinfo";

/// Reads the kernel's mount table. Injected, so the table can be a fixture.
type MountReader = Arc<dyn Fn() -> io::Result<String> + Send + Sync>;

fn read_mount_table() -> io::Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(MOUNT_TABLE)?).into_owned())
}

pub struct Platform {
    reader: MountReader,
    /// The file to wait on for changes of the mount table; none when the table is injected.
    watched_table: Option<PathBuf>,
    /// False when the plugin must not try UDisks2, which a test uses to be independent of the machine.
    use_udisks: bool,
    connection: tokio::sync::Mutex<Option<Connection>>,
    tasks: Mutex<Vec<AbortHandle>>,
    stop: Arc<AtomicBool>,
}

impl Platform {
    /// The real system.
    pub fn new() -> Self {
        Platform {
            reader: Arc::new(read_mount_table),
            watched_table: Some(PathBuf::from(MOUNT_TABLE)),
            use_udisks: true,
            connection: tokio::sync::Mutex::new(None),
            tasks: Mutex::new(Vec::new()),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The mount table alone, from `reader`.
    #[cfg(test)]
    fn mount_table_only(reader: MountReader) -> Self {
        Platform {
            reader,
            watched_table: None,
            use_udisks: false,
            connection: tokio::sync::Mutex::new(None),
            tasks: Mutex::new(Vec::new()),
            stop: Arc::new(AtomicBool::new(false)),
        }
    }

    /// The connection to UDisks2, or why there is none. A failure is not remembered: UDisks2 may be started later.
    async fn udisks(&self) -> std::result::Result<Connection, (Reason, String)> {
        if !self.use_udisks {
            return Err((
                Reason::NoSystemBus,
                "UDisks2 is not used on this platform object".to_string(),
            ));
        }
        let mut cached = self.connection.lock().await;
        if let Some(connection) = cached.as_ref() {
            return Ok(connection.clone());
        }
        let connection = match Connection::system().await {
            Ok(connection) => connection,
            Err(error) if Path::new("/.flatpak-info").exists() => {
                return Err((
                    Reason::FlatpakSandbox,
                    format!("this Flatpak cannot reach the system bus ({error}); grant --system-talk-name=org.freedesktop.UDisks2 to manage drives"),
                ))
            }
            Err(error) => {
                return Err((
                    Reason::NoSystemBus,
                    format!("the system bus is not reachable ({error})"),
                ))
            }
        };
        let present = async {
            let bus = fdo::DBusProxy::new(&connection).await?;
            let name: zbus::names::BusName<'_> = SERVICE.try_into()?;
            if bus.name_has_owner(name).await? {
                return Ok::<bool, zbus::Error>(true);
            }
            Ok(bus
                .list_activatable_names()
                .await?
                .iter()
                .any(|name| name.as_str() == SERVICE))
        }
        .await
        .unwrap_or(false);
        if !present {
            return Err((
                Reason::Udisks2Missing,
                "UDisks2 is not installed or cannot be started".to_string(),
            ));
        }
        *cached = Some(connection.clone());
        Ok(connection)
    }

    async fn snapshot(
        &self,
        connection: &Connection,
    ) -> std::result::Result<udisks::Snapshot, zbus::Error> {
        let proxy = fdo::ObjectManagerProxy::builder(connection)
            .destination(SERVICE)?
            .path(ROOT)?
            .build()
            .await?;
        Ok(snapshot_from(proxy.get_managed_objects().await?))
    }

    /// A connection and a fresh snapshot for an action, or the error the action should report.
    async fn for_action(&self) -> Result<(Connection, udisks::Snapshot)> {
        let connection = self.udisks().await.map_err(|_| VolumesError::Unsupported)?;
        let snapshot = self
            .snapshot(&connection)
            .await
            .map_err(|error| VolumesError::io(error.to_string()))?;
        Ok((connection, snapshot))
    }
}

impl Drop for Platform {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(tasks) = self.tasks.lock() {
            for task in tasks.iter() {
                task.abort();
            }
        }
    }
}

/// Reduces a value from the bus to what the pure model reads. Byte strings arrive as text with the trailing NUL dropped.
pub(crate) fn to_prop(value: &Value<'_>) -> Option<udisks::Prop> {
    use udisks::Prop;
    fn text(bytes: &Value<'_>) -> Option<String> {
        let Value::Array(items) = bytes else {
            return None;
        };
        let raw: Vec<u8> = items
            .iter()
            .filter_map(|byte| match byte {
                Value::U8(byte) => Some(*byte),
                _ => None,
            })
            .collect();
        let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
        Some(String::from_utf8_lossy(&raw[..end]).into_owned())
    }
    match value {
        Value::Bool(flag) => Some(Prop::Bool(*flag)),
        Value::U8(n) => Some(Prop::Uint(u64::from(*n))),
        Value::U16(n) => Some(Prop::Uint(u64::from(*n))),
        Value::U32(n) => Some(Prop::Uint(u64::from(*n))),
        Value::U64(n) => Some(Prop::Uint(*n)),
        Value::I32(n) => u64::try_from(*n).ok().map(Prop::Uint),
        Value::I64(n) => u64::try_from(*n).ok().map(Prop::Uint),
        Value::Str(text) => Some(Prop::Str(text.to_string())),
        Value::ObjectPath(path) => Some(Prop::Str(path.to_string())),
        Value::Value(inner) => to_prop(inner),
        Value::Array(items) => match items.element_signature().as_str() {
            "y" => text(value).map(Prop::Str),
            "ay" => Some(Prop::Strs(items.iter().filter_map(text).collect())),
            "s" | "o" => Some(Prop::Strs(
                items
                    .iter()
                    .filter_map(|item| match item {
                        Value::Str(text) => Some(text.to_string()),
                        Value::ObjectPath(path) => Some(path.to_string()),
                        _ => None,
                    })
                    .collect(),
            )),
            _ => None,
        },
        _ => None,
    }
}

fn snapshot_from(objects: fdo::ManagedObjects) -> udisks::Snapshot {
    let mut snapshot = udisks::Snapshot::default();
    for (path, interfaces) in objects {
        let mut by_interface = BTreeMap::new();
        for (interface, properties) in interfaces {
            let props: udisks::Props = properties
                .iter()
                .filter_map(|(name, value)| Some((name.clone(), to_prop(value)?)))
                .collect();
            by_interface.insert(interface.to_string(), props);
        }
        snapshot.objects.insert(path.to_string(), by_interface);
    }
    snapshot
}

/// The error name and message of a failed call, when the failure came from the other side.
fn remote_error(error: &zbus::Error) -> Option<(String, String)> {
    match error {
        zbus::Error::MethodError(name, description, _) => {
            Some((name.to_string(), description.clone().unwrap_or_default()))
        }
        zbus::Error::FDO(error) => Some((
            error.name().to_string(),
            error.description().unwrap_or_default().to_string(),
        )),
        _ => None,
    }
}

/// Maps a failed call to the typed error. A busy device is looked up in `/proc` for what holds it.
async fn failure(error: zbus::Error, mount_point: Option<&str>) -> VolumesError {
    let Some((name, message)) = remote_error(&error) else {
        return VolumesError::io(error.to_string());
    };
    let holder = if name == "org.freedesktop.UDisks2.Error.DeviceBusy" {
        match mount_point.map(str::to_string) {
            Some(mount) => {
                tokio::task::spawn_blocking(move || find_holders(Path::new("/proc"), &mount))
                    .await
                    .ok()
                    .flatten()
            }
            None => None,
        }
    } else {
        None
    };
    VolumesError::from_dbus(&name, &message, || holder)
}

/// The options of every call: ask polkit, which asks the person through the desktop's agent.
fn interactive() -> HashMap<&'static str, Value<'static>> {
    HashMap::from([("auth.no_user_interaction", Value::Bool(false))])
}

async fn call<B, R>(
    connection: &Connection,
    path: &str,
    interface: &str,
    method: &str,
    body: &B,
    mount_point: Option<&str>,
) -> Result<R>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    R: serde::de::DeserializeOwned + zbus::zvariant::Type,
{
    let proxy = Proxy::new(connection, SERVICE, path.to_string(), interface.to_string())
        .await
        .map_err(|error| VolumesError::io(error.to_string()))?;
    match proxy.call::<_, _, R>(method, body).await {
        Ok(reply) => Ok(reply),
        Err(error) => Err(failure(error, mount_point).await),
    }
}

async fn unmount_block(
    connection: &Connection,
    path: &str,
    mount_point: Option<&str>,
) -> Result<()> {
    call::<_, ()>(
        connection,
        path,
        udisks::FILESYSTEM,
        "Unmount",
        &(interactive(),),
        mount_point,
    )
    .await
}

/// Blocks until the mount table changes: the kernel flags the file with `POLLERR | POLLPRI` whenever a mount is added or removed. Runs on a thread of its own and stops with the platform.
fn watch_mount_table(path: PathBuf, stop: Arc<AtomicBool>, notify: Notify) {
    use std::io::{Read, Seek, SeekFrom};
    use std::os::fd::AsRawFd;

    let Ok(mut file) = std::fs::File::open(&path) else {
        return;
    };
    let mut buffer = Vec::new();
    while !stop.load(Ordering::Relaxed) {
        // Reading to the end re-arms the notification.
        buffer.clear();
        if file.seek(SeekFrom::Start(0)).is_err() || file.read_to_end(&mut buffer).is_err() {
            return;
        }
        let mut poll = libc::pollfd {
            fd: file.as_raw_fd(),
            events: libc::POLLERR | libc::POLLPRI,
            revents: 0,
        };
        // SAFETY: `poll` is one valid `pollfd` for a file that stays open for the call.
        let ready = unsafe { libc::poll(&mut poll, 1, 500) };
        if ready > 0 && poll.revents & (libc::POLLERR | libc::POLLPRI) != 0 {
            notify();
        }
    }
}

impl Backend for Platform {
    fn status(&self) -> BoxFuture<'_, PluginStatus> {
        Box::pin(async move {
            match self.udisks().await {
                Ok(_) => PluginStatus::build(
                    Flavour::Udisks2,
                    [
                        FEATURE_LIST,
                        FEATURE_MOUNT,
                        FEATURE_UNMOUNT,
                        FEATURE_EJECT,
                        FEATURE_UNLOCK,
                        FEATURE_WATCH,
                    ]
                    .into_iter()
                    .map(FeatureStatus::available)
                    .collect(),
                ),
                Err((reason, message)) => {
                    let off = |name| FeatureStatus::unavailable(name, reason, message.clone());
                    PluginStatus::build(
                        Flavour::Mountinfo,
                        vec![
                            FeatureStatus::available(FEATURE_LIST),
                            off(FEATURE_MOUNT),
                            off(FEATURE_UNMOUNT),
                            off(FEATURE_EJECT),
                            off(FEATURE_UNLOCK),
                            FeatureStatus::available(FEATURE_WATCH),
                        ],
                    )
                }
            }
        })
    }

    fn volumes(&self) -> BoxFuture<'_, Result<Vec<Volume>>> {
        Box::pin(async move {
            let table = (self.reader)().map_err(|error| VolumesError::io(error.to_string()))?;
            let entries = mountinfo::parse(&table);
            if let Ok(connection) = self.udisks().await {
                match self.snapshot(&connection).await {
                    Ok(snapshot) => {
                        let mut volumes = udisks::volumes_from_snapshot(&snapshot);
                        volumes.extend(mountinfo::volumes_from_mounts(&entries, false));
                        return Ok(volumes);
                    }
                    Err(error) => {
                        log::warn!("volumes: UDisks2 would not list its objects, using the mount table: {error}")
                    }
                }
            }
            Ok(mountinfo::volumes_from_mounts(&entries, true))
        })
    }

    fn mount(&self, id: String) -> BoxFuture<'_, Result<String>> {
        Box::pin(async move {
            let (connection, snapshot) = self.for_action().await?;
            let target = udisks::filesystem_target(&snapshot, &id)?;
            if let Some(mount_point) = target.mount_point {
                return Ok(mount_point);
            }
            call::<_, String>(
                &connection,
                &target.path,
                udisks::FILESYSTEM,
                "Mount",
                &(interactive(),),
                None,
            )
            .await
        })
    }

    fn unmount(&self, id: String) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let (connection, snapshot) = self.for_action().await?;
            let target = udisks::filesystem_target(&snapshot, &id)?;
            unmount_block(&connection, &target.path, target.mount_point.as_deref()).await
        })
    }

    fn eject(&self, id: String) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let (connection, snapshot) = self.for_action().await?;
            let plan = udisks::plan_eject(&snapshot, &id)?;
            for (path, mount_point) in &plan.unmount {
                unmount_block(&connection, path, Some(mount_point)).await?;
            }
            for path in &plan.lock {
                call::<_, ()>(
                    &connection,
                    path,
                    udisks::ENCRYPTED,
                    "Lock",
                    &(interactive(),),
                    None,
                )
                .await?;
            }
            let mut ejected = false;
            if let Some(drive) = &plan.eject {
                call::<_, ()>(
                    &connection,
                    drive,
                    udisks::DRIVE,
                    "Eject",
                    &(interactive(),),
                    None,
                )
                .await?;
                ejected = true;
            }
            if let Some(drive) = &plan.power_off {
                let result = call::<_, ()>(
                    &connection,
                    drive,
                    udisks::DRIVE,
                    "PowerOff",
                    &(interactive(),),
                    None,
                )
                .await;
                // After an eject the drive may already be gone, which is what was asked for.
                match result {
                    Err(VolumesError::NotFound | VolumesError::Unsupported) if ejected => {}
                    other => other?,
                }
            }
            Ok(())
        })
    }

    fn unlock(&self, id: String, passphrase: Passphrase) -> BoxFuture<'_, Result<String>> {
        Box::pin(async move {
            let (connection, snapshot) = self.for_action().await?;
            let path = udisks::unlock_target(&snapshot, &id)?;
            let cleartext: OwnedObjectPath = call(
                &connection,
                &path,
                udisks::ENCRYPTED,
                "Unlock",
                &(passphrase.0.as_str(), interactive()),
                None,
            )
            .await?;
            Ok(udisks::id_of(cleartext.as_str()))
        })
    }

    fn watch(&self, notify: Notify) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            if let Some(path) = self.watched_table.clone() {
                let stop = Arc::clone(&self.stop);
                let notify = Arc::clone(&notify);
                let spawned = std::thread::Builder::new()
                    .name("volumes-mountinfo".into())
                    .spawn(move || watch_mount_table(path, stop, notify));
                if let Err(error) = spawned {
                    log::warn!("volumes: could not watch the mount table: {error}");
                }
            }
            let Ok(connection) = self.udisks().await else {
                return Ok(());
            };
            let rule = MatchRule::builder()
                .msg_type(MessageType::Signal)
                .path_namespace(ROOT)
                .map_err(|error| VolumesError::io(error.to_string()))?
                .build();
            let mut signals = MessageStream::for_match_rule(rule, &connection, Some(256))
                .await
                .map_err(|error| VolumesError::io(error.to_string()))?;
            let task = tokio::spawn(async move {
                while let Some(message) = signals.next().await {
                    let Ok(message) = message else { continue };
                    let header = message.header();
                    let interface = header.interface().map(|name| name.as_str());
                    if matches!(
                        interface,
                        Some(
                            "org.freedesktop.DBus.ObjectManager"
                                | "org.freedesktop.DBus.Properties"
                        )
                    ) {
                        notify();
                    }
                }
            });
            if let Ok(mut tasks) = self.tasks.lock() {
                tasks.push(task.abort_handle());
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Array;

    fn mount_table() -> Platform {
        Platform::mount_table_only(Arc::new(|| {
            Ok(include_str!("../tests/fixtures/mountinfo.txt").to_string())
        }))
    }

    #[tokio::test]
    async fn without_udisks_the_status_lists_and_watches_but_cannot_act() {
        let status = mount_table().status().await;
        assert_eq!(status.flavour, Flavour::Mountinfo);
        assert!(status.available);
        assert!(status.has(FEATURE_LIST) && status.has(FEATURE_WATCH));
        for feature in [
            FEATURE_MOUNT,
            FEATURE_UNMOUNT,
            FEATURE_EJECT,
            FEATURE_UNLOCK,
        ] {
            assert!(!status.has(feature), "{feature}");
        }
        let mount = status
            .features
            .iter()
            .find(|f| f.name == FEATURE_MOUNT)
            .unwrap();
        assert_eq!(mount.reason, Some(Reason::NoSystemBus));
        assert!(mount.message.is_some());
        assert_eq!(status.reason, Some(Reason::NoSystemBus));
    }

    #[tokio::test]
    async fn without_udisks_the_list_is_the_mount_table_with_its_disks() {
        let volumes = mount_table().volumes().await.unwrap();
        assert!(volumes.iter().any(|v| v.id == "mount:/"));
        assert!(volumes.iter().any(|v| v.id == "mount:/mnt/nas"));
    }

    #[tokio::test]
    async fn without_udisks_every_action_is_unsupported() {
        let platform = mount_table();
        assert_eq!(
            platform.mount("udisks2:sdb1".into()).await,
            Err(VolumesError::Unsupported)
        );
        assert_eq!(
            platform.unmount("udisks2:sdb1".into()).await,
            Err(VolumesError::Unsupported)
        );
        assert_eq!(
            platform.eject("udisks2:sdb1".into()).await,
            Err(VolumesError::Unsupported)
        );
        assert_eq!(
            platform
                .unlock("udisks2:sdd1".into(), Passphrase("x".into()))
                .await,
            Err(VolumesError::Unsupported)
        );
    }

    #[tokio::test]
    async fn an_unreadable_mount_table_is_an_error_not_an_empty_list() {
        let platform = Platform::mount_table_only(Arc::new(|| {
            Err(io::Error::new(io::ErrorKind::NotFound, "no /proc"))
        }));
        assert!(matches!(
            platform.volumes().await,
            Err(VolumesError::Io { .. })
        ));
    }

    #[test]
    fn bus_values_reduce_to_the_pure_models_properties() {
        use udisks::Prop;
        assert_eq!(to_prop(&Value::Bool(true)), Some(Prop::Bool(true)));
        assert_eq!(to_prop(&Value::U64(7)), Some(Prop::Uint(7)));
        assert_eq!(to_prop(&Value::new("x")), Some(Prop::Str("x".into())));
        let bytes = Value::from(Array::from(&b"/dev/sda1\0"[..]));
        assert_eq!(to_prop(&bytes), Some(Prop::Str("/dev/sda1".into())));
        let mounts = vec![b"/mnt/a\0".to_vec(), b"/mnt/b c\0".to_vec()];
        assert_eq!(
            to_prop(&Value::from(mounts)),
            Some(Prop::Strs(vec!["/mnt/a".into(), "/mnt/b c".into()]))
        );
        let empty: Vec<Vec<u8>> = Vec::new();
        assert_eq!(to_prop(&Value::from(empty)), Some(Prop::Strs(vec![])));
    }

    /// Prints this machine's real volumes. Read-only: it asks UDisks2 for its objects and reads the mount table, and never mounts, unmounts, ejects or unlocks anything. Run it by hand: `cargo test -p tauri-plugin-volumes live_list -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "reads the real system"]
    async fn live_list() {
        let platform = Platform::new();
        let status = platform.status().await;
        println!(
            "flavour: {:?}, available: {}",
            status.flavour, status.available
        );
        for feature in &status.features {
            println!(
                "  {:<8} {} {:?}",
                feature.name, feature.available, feature.reason
            );
        }
        let mut volumes = platform.volumes().await.expect("the volumes list");
        for volume in &mut volumes {
            if volume.kind != crate::models::VolumeKind::Network {
                if let Some(space) = volume
                    .mount_point
                    .as_deref()
                    .and_then(crate::space::system_space)
                {
                    volume.total = Some(space.total);
                    volume.free = Some(space.free);
                }
            }
            println!(
                "{:<22} {:<10?} {:<26} fs={:<10} at={:<28} total={:?} free={:?} mount={} unmount={} eject={} locked={} system={} dev={:?}",
                volume.id,
                volume.kind,
                volume.label,
                volume.file_system.as_deref().unwrap_or("-"),
                volume.mount_point.as_deref().unwrap_or("-"),
                volume.total,
                volume.free,
                volume.can_mount,
                volume.can_unmount,
                volume.can_eject,
                volume.locked,
                volume.is_system,
                volume.device,
            );
        }
    }
}
