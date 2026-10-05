// Importing settings as a plan first and an action second, over the configuration files that exist
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT
//
// A configuration file is anything a person configures that is exported and imported: the
// settings document, the operations settings. Each is a `ConfigFile` with an id; adding one to
// the export is implementing the trait and registering it (see `docs/architecture/`). Importing
// asks every file in the bundle to `plan`: validate the incoming document exactly as a normal
// change would, and say what would differ. Nothing is touched until `apply_import`, which puts
// the planned documents in force and puts every one back if a later one fails.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::bundle::{read, BundleError, BundleKind};
use crate::model::Settings;
use crate::storage::{SettingsDocument, DOCUMENT_VERSION};

/// The id the settings document is exported under.
pub const SETTINGS_FILE_ID: &str = "settings";

/// What importing one configuration file would do.
#[derive(Debug, Clone, PartialEq)]
pub struct FilePlan {
    /// The document to put in force: what was read, brought to the shape this build writes.
    pub document: Value,
    pub changes: Vec<ChangeGroup>,
    pub warnings: Vec<ImportWarning>,
}

/// One configuration file that can be exported and imported.
pub trait ConfigFile: Send + Sync {
    /// The id the file is exported under: lower-case letters, digits and hyphens.
    fn id(&self) -> &str;

    /// The document in force now, as it is exported.
    fn export(&self) -> Result<Value, String>;

    /// Checks `incoming` as a normal change would and says what it would change, touching
    /// nothing. A value that a normal change would refuse is refused here.
    fn plan(&self, incoming: &Value) -> Result<FilePlan, BundleError>;

    /// Puts `document` (one `plan` returned) in force, as a replacement of what is there.
    fn apply(&self, document: &Value) -> Result<(), String>;

    /// Puts back a document `export` returned after a later file failed to apply. Like `apply`
    /// unless putting back should leave no trace of its own.
    fn restore(&self, document: &Value) -> Result<(), String> {
        self.apply(document)
    }
}

/// A settings group (`general`, `dnd`, …) of a file and how many of its values would change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ChangeGroup {
    /// The configuration file's id.
    pub file: String,
    /// The group within it: a top-level key of the document's body.
    pub group: String,
    pub count: u32,
}

/// Something about the file the person should know before replacing their settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub enum ImportWarning {
    /// Settings this version does not have, which are left out (a file from a newer Waypoint).
    UnknownKeys { file: String, keys: Vec<String> },
    /// Configuration files in the bundle that this version does not have, which are left out.
    UnknownFiles { ids: Vec<String> },
    /// Zip entries the manifest does not list, which were not read.
    UnlistedEntries { names: Vec<String> },
}

/// What importing a file would do, for the person to confirm. The page gets this and nothing
/// that it could hand back as the thing to apply: Rust keeps the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ImportPlan {
    pub kind: BundleKind,
    /// The Waypoint version that made the file, when it says.
    pub app_version: String,
    /// When the file was made (UTC, RFC 3339), when it says.
    pub exported_at: String,
    /// The configuration files that will be replaced.
    pub files: Vec<String>,
    pub changes: Vec<ChangeGroup>,
    pub warnings: Vec<ImportWarning>,
}

/// A plan with the number the page hands back to apply it. Rust keeps the file the plan was made
/// from; the number only says which one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ImportPreview {
    #[ts(type = "number")]
    pub plan_id: u64,
    pub plan: ImportPlan,
}

/// Where an export was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../packages/protocol/src/generated/")]
pub struct ExportReceipt {
    pub path: String,
    pub kind: BundleKind,
    /// The configuration files that are in it.
    pub files: Vec<String>,
}

/// A plan and the documents it would apply.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedImport {
    pub plan: ImportPlan,
    pub documents: Vec<(String, Value)>,
}

/// Reads `bytes` and plans importing them over `files`, touching nothing. Bundle files that no
/// `ConfigFile` has are left out with a warning; at least one must be recognised.
pub fn plan_import(
    bytes: &[u8],
    files: &[Arc<dyn ConfigFile>],
) -> Result<PlannedImport, BundleError> {
    let bundle = read(bytes)?;
    let mut documents = Vec::new();
    let mut changes = Vec::new();
    let mut warnings = Vec::new();
    let mut ids = Vec::new();
    let mut unknown = Vec::new();
    for incoming in &bundle.files {
        let Some(target) = files.iter().find(|f| f.id() == incoming.id) else {
            unknown.push(incoming.id.clone());
            continue;
        };
        let planned = target.plan(&incoming.document)?;
        ids.push(incoming.id.clone());
        documents.push((incoming.id.clone(), planned.document));
        changes.extend(planned.changes);
        warnings.extend(planned.warnings);
    }
    if ids.is_empty() {
        return Err(BundleError::NothingRecognised);
    }
    if !unknown.is_empty() {
        warnings.push(ImportWarning::UnknownFiles { ids: unknown });
    }
    if !bundle.unlisted_entries.is_empty() {
        warnings.push(ImportWarning::UnlistedEntries {
            names: bundle.unlisted_entries,
        });
    }
    Ok(PlannedImport {
        plan: ImportPlan {
            kind: bundle.kind,
            app_version: bundle.app_version,
            exported_at: bundle.exported_at,
            files: ids,
            changes,
            warnings,
        },
        documents,
    })
}

/// Why applying failed, and whether what was applied before it was put back.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("could not apply `{file}`: {message}{}", if *.restored { "" } else { " (the earlier files could not all be put back)" })]
pub struct ApplyError {
    pub file: String,
    pub message: String,
    pub restored: bool,
}

/// Puts the planned documents in force, all or none: when one file fails, every file applied
/// before it is put back as it was.
pub fn apply_import(
    planned: &PlannedImport,
    files: &[Arc<dyn ConfigFile>],
) -> Result<(), ApplyError> {
    let mut applied: Vec<(&Arc<dyn ConfigFile>, Value)> = Vec::new();
    for (id, document) in &planned.documents {
        let failure = |message: String, applied: &mut Vec<(&Arc<dyn ConfigFile>, Value)>| {
            let mut restored = true;
            while let Some((target, before)) = applied.pop() {
                if let Err(why) = target.restore(&before) {
                    log::error!(
                        "could not put back `{}` after a failed import: {why}",
                        target.id()
                    );
                    restored = false;
                }
            }
            ApplyError {
                file: id.clone(),
                message,
                restored,
            }
        };
        let Some(target) = files.iter().find(|f| f.id() == id) else {
            return Err(failure(
                "no such configuration file".to_owned(),
                &mut applied,
            ));
        };
        let before = match target.export() {
            Ok(before) => before,
            Err(why) => return Err(failure(why, &mut applied)),
        };
        match target.apply(document) {
            Ok(()) => applied.push((target, before)),
            Err(why) => return Err(failure(why, &mut applied)),
        }
    }
    Ok(())
}

/// The paths (`dnd.springLoadMs`) of every value that differs between `a` and `b`.
pub fn differing_paths(a: &Value, b: &Value) -> Vec<String> {
    fn walk(a: &Value, b: &Value, path: &str, out: &mut Vec<String>) {
        match (a, b) {
            (Value::Object(x), Value::Object(y)) => {
                let mut keys: Vec<&String> = x.keys().chain(y.keys()).collect();
                keys.sort();
                keys.dedup();
                for key in keys {
                    let child = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    };
                    match (x.get(key), y.get(key)) {
                        (Some(l), Some(r)) => walk(l, r, &child, out),
                        _ => out.push(child),
                    }
                }
            }
            _ if a != b => out.push(path.to_owned()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(a, b, "", &mut out);
    out
}

/// The paths of every key in `incoming` that `known` (the same document as this build writes it)
/// does not have.
pub fn unknown_paths(incoming: &Value, known: &Value) -> Vec<String> {
    fn walk(incoming: &Value, known: &Value, path: &str, out: &mut Vec<String>) {
        let (Value::Object(i), Value::Object(k)) = (incoming, known) else {
            return;
        };
        for (key, value) in i {
            let child = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            match k.get(key) {
                Some(known) => walk(value, known, &child, out),
                None => out.push(child),
            }
        }
    }
    let mut out = Vec::new();
    walk(incoming, known, "", &mut out);
    out
}

/// Groups the differing paths by their first part, the way the Settings pages group them.
/// `skip` is a leading path part to look past (`body` in a document with a version).
pub fn change_groups(file: &str, paths: &[String], skip: Option<&str>) -> Vec<ChangeGroup> {
    let mut groups: Vec<ChangeGroup> = Vec::new();
    for path in paths {
        let mut parts = path.split('.');
        let first = parts.next().unwrap_or_default();
        let group = match skip {
            Some(skipped) if first == skipped => parts.next().unwrap_or(first),
            _ => first,
        };
        match groups.iter_mut().find(|g| g.group == group) {
            Some(existing) => existing.count += 1,
            None => groups.push(ChangeGroup {
                file: file.to_owned(),
                group: group.to_owned(),
                count: 1,
            }),
        }
    }
    groups
}

/// Plans importing a settings document over `current`: a `SettingsDocument` (`version`, `body`)
/// that a normal change would accept, with keys this build does not have left out and noted.
pub fn plan_settings(current: &Settings, incoming: &Value) -> Result<FilePlan, BundleError> {
    let invalid = |message: String| BundleError::Invalid {
        file: SETTINGS_FILE_ID.to_owned(),
        message,
    };
    let object = incoming
        .as_object()
        .ok_or_else(|| invalid("it is not an object".to_owned()))?;
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid("it has no version".to_owned()))?;
    if version > u64::from(DOCUMENT_VERSION) {
        return Err(BundleError::NewerDocument {
            file: SETTINGS_FILE_ID.to_owned(),
            found: version,
            supported: DOCUMENT_VERSION,
        });
    }
    let body = object
        .get("body")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    let settings: Settings =
        serde_json::from_value(body.clone()).map_err(|e| invalid(e.to_string()))?;
    settings.validate().map_err(|e| invalid(e.to_string()))?;

    let document = serde_json::to_value(SettingsDocument::new(settings))
        .map_err(|e| invalid(e.to_string()))?;
    let now = serde_json::to_value(current).map_err(|e| invalid(e.to_string()))?;
    let incoming_body = &document["body"];
    let changes = change_groups(
        SETTINGS_FILE_ID,
        &differing_paths(&now, incoming_body),
        None,
    );
    let unknown = unknown_paths(&body, incoming_body);
    let warnings = if unknown.is_empty() {
        Vec::new()
    } else {
        vec![ImportWarning::UnknownKeys {
            file: SETTINGS_FILE_ID.to_owned(),
            keys: unknown,
        }]
    };
    Ok(FilePlan {
        document,
        changes,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use serde_json::json;

    use super::*;
    use crate::bundle::{export, ExportFile, ExportMeta};

    fn meta() -> ExportMeta {
        ExportMeta {
            app_version: "0.1.0".to_owned(),
            exported_at_unix: 1_790_994_600,
            local_offset_minutes: 0,
        }
    }

    fn document(settings: &Settings) -> Value {
        serde_json::to_value(SettingsDocument::new(settings.clone())).unwrap()
    }

    /// A configuration file kept in memory, with a switch to make its next apply fail.
    struct Fake {
        id: &'static str,
        value: Mutex<Value>,
        fail_apply: Mutex<bool>,
        applied: Mutex<u32>,
    }

    impl Fake {
        fn new(id: &'static str, value: Value) -> Arc<Self> {
            Arc::new(Self {
                id,
                value: Mutex::new(value),
                fail_apply: Mutex::new(false),
                applied: Mutex::new(0),
            })
        }
    }

    impl ConfigFile for Fake {
        fn id(&self) -> &str {
            self.id
        }
        fn export(&self) -> Result<Value, String> {
            Ok(self.value.lock().unwrap().clone())
        }
        fn plan(&self, incoming: &Value) -> Result<FilePlan, BundleError> {
            let now = self.value.lock().unwrap().clone();
            Ok(FilePlan {
                document: incoming.clone(),
                changes: change_groups(self.id, &differing_paths(&now, incoming), None),
                warnings: Vec::new(),
            })
        }
        fn apply(&self, document: &Value) -> Result<(), String> {
            if *self.fail_apply.lock().unwrap() {
                return Err("disk full".to_owned());
            }
            *self.applied.lock().unwrap() += 1;
            *self.value.lock().unwrap() = document.clone();
            Ok(())
        }
    }

    struct SettingsFake(Mutex<Settings>);

    impl ConfigFile for SettingsFake {
        fn id(&self) -> &str {
            SETTINGS_FILE_ID
        }
        fn export(&self) -> Result<Value, String> {
            Ok(document(&self.0.lock().unwrap()))
        }
        fn plan(&self, incoming: &Value) -> Result<FilePlan, BundleError> {
            plan_settings(&self.0.lock().unwrap(), incoming)
        }
        fn apply(&self, document: &Value) -> Result<(), String> {
            let doc: SettingsDocument = serde_json::from_value(document.clone()).unwrap();
            *self.0.lock().unwrap() = doc.body;
            Ok(())
        }
    }

    fn bundle_of(settings: &Settings) -> Vec<u8> {
        export(
            &[ExportFile {
                id: "settings".to_owned(),
                document: document(settings),
            }],
            &meta(),
        )
        .unwrap()
        .bytes
    }

    fn settings_file(settings: Settings) -> Arc<dyn ConfigFile> {
        Arc::new(SettingsFake(Mutex::new(settings)))
    }

    #[test]
    fn the_plan_lists_the_groups_that_would_change_with_counts() {
        let mut incoming = Settings::default();
        incoming.general.show_hidden_default = true;
        incoming.general.click_mode = crate::ClickMode::Single;
        incoming.dnd.spring_load_ms = 900;
        let planned =
            plan_import(&bundle_of(&incoming), &[settings_file(Settings::default())]).unwrap();
        assert_eq!(planned.plan.kind, BundleKind::Json);
        assert_eq!(planned.plan.files, ["settings"]);
        assert_eq!(planned.plan.app_version, "0.1.0");
        assert_eq!(
            planned.plan.changes,
            [
                ChangeGroup {
                    file: "settings".into(),
                    group: "dnd".into(),
                    count: 1
                },
                ChangeGroup {
                    file: "settings".into(),
                    group: "general".into(),
                    count: 2
                },
            ]
        );
        assert!(planned.plan.warnings.is_empty());
    }

    #[test]
    fn the_experimental_switches_travel_in_the_export_and_come_back_on_import() {
        let mut incoming = Settings::default();
        incoming.experimental.sftp = true;
        incoming.experimental.webdav = true;
        let bundle = bundle_of(&incoming);
        let planned = plan_import(&bundle, &[settings_file(Settings::default())]).unwrap();
        assert_eq!(
            planned.plan.changes,
            [ChangeGroup {
                file: "settings".into(),
                group: "experimental".into(),
                count: 2
            }]
        );
        assert!(planned.plan.warnings.is_empty());
        let (_, document) = planned
            .documents
            .iter()
            .find(|(id, _)| id == "settings")
            .unwrap();
        assert_eq!(
            document["body"]["experimental"],
            serde_json::json!({"sftp": true, "smb": false, "webdav": true, "s3": false})
        );
    }

    #[test]
    fn importing_what_is_already_in_force_changes_nothing() {
        let planned = plan_import(
            &bundle_of(&Settings::default()),
            &[settings_file(Settings::default())],
        )
        .unwrap();
        assert!(planned.plan.changes.is_empty());
    }

    #[test]
    fn the_plan_touches_nothing() {
        let file = settings_file(Settings::default());
        let mut incoming = Settings::default();
        incoming.dnd.spring_load_ms = 900;
        plan_import(&bundle_of(&incoming), std::slice::from_ref(&file)).unwrap();
        assert_eq!(file.export().unwrap(), document(&Settings::default()));
    }

    #[test]
    fn keys_this_build_does_not_have_are_left_out_and_noted() {
        let mut value = document(&Settings::default());
        value["body"]["general"]["brandNewThing"] = json!(true);
        value["body"]["futureSection"] = json!({ "a": 1 });
        let bytes = export(
            &[ExportFile {
                id: "settings".into(),
                document: value,
            }],
            &meta(),
        )
        .unwrap()
        .bytes;
        let planned = plan_import(&bytes, &[settings_file(Settings::default())]).unwrap();
        assert_eq!(
            planned.plan.warnings,
            [ImportWarning::UnknownKeys {
                file: "settings".into(),
                keys: vec!["futureSection".into(), "general.brandNewThing".into()],
            }]
        );
        assert!(planned.documents[0].1["body"]
            .get("futureSection")
            .is_none());
    }

    #[test]
    fn values_a_normal_change_refuses_are_refused_with_the_field() {
        let mut value = document(&Settings::default());
        value["body"]["dnd"]["springLoadMs"] = json!(5);
        let bytes = export(
            &[ExportFile {
                id: "settings".into(),
                document: value,
            }],
            &meta(),
        )
        .unwrap()
        .bytes;
        let error = plan_import(&bytes, &[settings_file(Settings::default())]).unwrap_err();
        assert!(
            matches!(&error, BundleError::Invalid { message, .. } if message.contains("dnd.springLoadMs")),
            "{error:?}"
        );
        assert_eq!(error.code(), "invalid");
    }

    #[test]
    fn a_value_of_the_wrong_type_is_refused() {
        let mut value = document(&Settings::default());
        value["body"]["general"]["defaultView"] = json!(7);
        let bytes = export(
            &[ExportFile {
                id: "settings".into(),
                document: value,
            }],
            &meta(),
        )
        .unwrap()
        .bytes;
        assert!(matches!(
            plan_import(&bytes, &[settings_file(Settings::default())]),
            Err(BundleError::Invalid { .. })
        ));
    }

    #[test]
    fn a_settings_document_from_a_newer_build_is_refused_with_its_version() {
        let value = json!({ "version": 2, "body": {} });
        let bytes = export(
            &[ExportFile {
                id: "settings".into(),
                document: value,
            }],
            &meta(),
        )
        .unwrap()
        .bytes;
        let error = plan_import(&bytes, &[settings_file(Settings::default())]).unwrap_err();
        assert_eq!(
            error,
            BundleError::NewerDocument {
                file: "settings".into(),
                found: 2,
                supported: 1
            }
        );
        assert_eq!(error.code(), "newer-format");
    }

    #[test]
    fn a_bundle_with_nothing_this_build_has_is_refused_and_unknown_files_are_noted() {
        let bytes = export(
            &[ExportFile {
                id: "future".into(),
                document: json!({}),
            }],
            &meta(),
        )
        .unwrap()
        .bytes;
        assert_eq!(
            plan_import(&bytes, &[settings_file(Settings::default())]).unwrap_err(),
            BundleError::NothingRecognised
        );
        let both = export(
            &[
                ExportFile {
                    id: "settings".into(),
                    document: document(&Settings::default()),
                },
                ExportFile {
                    id: "future".into(),
                    document: json!({}),
                },
            ],
            &meta(),
        )
        .unwrap()
        .bytes;
        let planned = plan_import(&both, &[settings_file(Settings::default())]).unwrap();
        assert_eq!(
            planned.plan.warnings,
            [ImportWarning::UnknownFiles {
                ids: vec!["future".into()]
            }]
        );
        assert_eq!(planned.plan.kind, BundleKind::Zip);
    }

    #[test]
    fn a_zip_of_two_files_plans_both_and_applies_both() {
        let settings = SettingsFake(Mutex::new(Settings::default()));
        let ops = Fake::new("ops", json!({ "concurrency": 2 }));
        let files: Vec<Arc<dyn ConfigFile>> = vec![Arc::new(settings), ops.clone()];
        let mut incoming = Settings::default();
        incoming.dnd.spring_load_ms = 900;
        let bytes = export(
            &[
                ExportFile {
                    id: "settings".into(),
                    document: document(&incoming),
                },
                ExportFile {
                    id: "ops".into(),
                    document: json!({ "concurrency": 4 }),
                },
            ],
            &meta(),
        )
        .unwrap()
        .bytes;
        let planned = plan_import(&bytes, &files).unwrap();
        assert_eq!(planned.plan.files, ["settings", "ops"]);
        assert_eq!(planned.plan.changes.len(), 2);
        apply_import(&planned, &files).unwrap();
        assert_eq!(files[0].export().unwrap(), document(&incoming));
        assert_eq!(ops.export().unwrap(), json!({ "concurrency": 4 }));
    }

    #[test]
    fn a_failure_part_way_puts_the_earlier_files_back() {
        let first = Fake::new("a", json!({ "n": 1 }));
        let second = Fake::new("b", json!({ "n": 1 }));
        *second.fail_apply.lock().unwrap() = true;
        let files: Vec<Arc<dyn ConfigFile>> = vec![first.clone(), second.clone()];
        let planned = PlannedImport {
            plan: ImportPlan {
                kind: BundleKind::Zip,
                app_version: String::new(),
                exported_at: String::new(),
                files: vec!["a".into(), "b".into()],
                changes: Vec::new(),
                warnings: Vec::new(),
            },
            documents: vec![
                ("a".into(), json!({ "n": 2 })),
                ("b".into(), json!({ "n": 2 })),
            ],
        };
        let error = apply_import(&planned, &files).unwrap_err();
        assert_eq!(error.file, "b");
        assert!(error.restored);
        assert_eq!(first.export().unwrap(), json!({ "n": 1 }), "put back");
        assert_eq!(second.export().unwrap(), json!({ "n": 1 }));
    }

    #[test]
    fn the_exported_documents_round_trip_to_a_plan_with_no_changes() {
        let mut settings = Settings::default();
        settings.dnd.spring_load_ms = 1200;
        settings.appearance.accent = crate::model::AccentChoice::Custom {
            hex: "#aabbcc".into(),
        };
        let bytes = bundle_of(&settings);
        let planned = plan_import(&bytes, &[settings_file(settings.clone())]).unwrap();
        assert!(planned.plan.changes.is_empty());
        assert!(planned.plan.warnings.is_empty());
        assert_eq!(planned.documents[0].1, document(&settings));
    }

    #[test]
    fn differing_and_unknown_paths_walk_nested_objects() {
        let a = json!({ "x": { "y": 1, "z": 2 }, "w": [1] });
        let b = json!({ "x": { "y": 1, "z": 3 }, "w": [2], "v": 0 });
        assert_eq!(differing_paths(&a, &b), ["v", "w", "x.z"]);
        assert_eq!(unknown_paths(&b, &a), ["v"]);
    }
}
