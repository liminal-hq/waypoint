// Waypoint's operations engine: jobs, the planner, the queue and the executors for the simple
// operations (create, rename, duplicate, trash, restore, delete). It is pure: no `tauri`, no
// threads of its own, and no access to the file system except through a `waypoint_vfs::Provider`.
// The copy and move engine, the undo journal and the Tauri plugin build on it in later slices.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod model;
mod names;
mod plan;
mod queue;
mod traits;

pub use model::*;
pub use names::{
    file_name_of, fold_name, is_within, same_name, same_path, split_name, unique_full_name,
    unique_name,
};
pub use plan::{plan, plan_with_progress, Plan, PlanCtx, PlanItem, PlanProgress, PlanWarning};
pub use queue::{is_legal, OpsStore, ProgressGate, QueueError};
pub use traits::{
    Clock, CounterIds, IdSource, Protected, Providers, SelectionResolver, SettingsReader,
    StaticSettings, SystemClock, Trash, TrashReceipt,
};
