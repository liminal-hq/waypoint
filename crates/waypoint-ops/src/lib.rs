// Waypoint's operations engine: jobs, the planner, the queue and the executors for the simple
// operations (create, rename, duplicate, trash, restore, delete) and for copy and move, with their
// conflict policies, error decisions and verification. It is pure: no `tauri`, no threads of its
// own, and no access to the file system except through a `waypoint_vfs::Provider`. The undo
// journal records what each job did and undoes it; the Tauri plugin builds on it in a later slice.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod checksum;
pub mod exec;
mod journal;
mod model;
mod names;
mod plan;
mod preview;
mod queue;
mod rename_clash;
mod rename_rules;
mod schedule;
mod speed;
mod stage;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod throttle;
mod traits;
mod verify;

pub use checksum::{
    checksum_reader, run_checksum, ChecksumEvent, REPORT_EVERY as CHECKSUM_REPORT_EVERY,
};
pub use exec::{
    action_for, remove_all, Action, CopyFile, CopyRequest, ExecEnv, ExecFailure, ExecReport,
    ExecSink, Executor, NullSink, Resolutions, RunOptions, SimpleCopy, TransferReport, CHUNK_BYTES,
};
pub use journal::*;
pub use model::*;
pub use names::{
    file_name_of, fold_name, is_within, same_name, same_path, split_name, unique_full_name,
    unique_name,
};
pub use plan::{
    plan, plan_with_progress, preview_batch, BatchPlan, BatchStep, CompressPlan, ExtractPlan,
    LeftOut, Plan, PlanCtx, PlanItem, PlanProgress, PlanWarning,
};
pub use preview::{
    conflict_preview, ConflictPreview, DiffLine, PreviewKind, PreviewSide, TextDiff,
    PREVIEW_CONTEXT, PREVIEW_DIFF_BUDGET, PREVIEW_MAX_BYTES, PREVIEW_MAX_LINES,
    PREVIEW_MAX_LINE_CHARS, PREVIEW_MAX_ROWS,
};
pub use queue::{is_legal, OpsStore, ProgressGate, QueueError};
pub use rename_clash::{
    apply_rules, plan_batch, preview_batch_rename, BatchEntry, BatchPreview, Clash, FolderInputs,
    Place, PreviewRequest, PreviewRow, Problem, RenameOutput, RenameStep, RuleOutputs,
};
pub use rename_rules::{
    validate_rules, CaseMode, DateSource, InsertAt, RenameCtx, RenameInput, RenameRule,
    RenameScope, RenameSpec, RuleError, RulePosition,
};
pub use schedule::Schedule;
pub use speed::SpeedEstimator;
pub use stage::{stage_files, STAGE_LIMIT_BYTES};
pub use throttle::{Bucket, Pacer, RateCell, SystemPacer, Throttle, WAIT_SLICE};
pub use traits::{
    Clock, CounterIds, IdSource, Protected, Providers, SelectionResolver, SettingsReader,
    StaticSettings, SystemClock, Trash, TrashReceipt,
};
pub use verify::{digest_of, hex, Hasher, Manifest};
