//! Fork / merge / entry updates over [`crate::ipld::WorkspaceRoot`].

use crate::ipld::WorkspaceRoot;
use crate::store::ContentStore;
use anyhow::{anyhow, Context, Result};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("merge conflict on slot {0:?}")]
    MergeConflict(String),
}

pub fn load_workspace(store: &dyn ContentStore, root_cid: &str) -> Result<WorkspaceRoot> {
    let v = store.dag_get_json(root_cid)?;
    let w = WorkspaceRoot::from_json_value(&v).context("parse workspace manifest")?;
    w.validate().map_err(|m| anyhow!("invalid workspace: {}", m))?;
    Ok(w)
}

pub fn save_workspace(store: &dyn ContentStore, w: &WorkspaceRoot) -> Result<String> {
    w.validate().map_err(|m| anyhow!("invalid workspace: {}", m))?;
    let v = w.to_json_value()?;
    store.dag_put_json(&v)
}

/// New workspace with same entries; `parents` set to `[parent_cid]` (fork lineage).
pub fn fork_workspace(parent: &WorkspaceRoot, parent_cid: &str) -> WorkspaceRoot {
    WorkspaceRoot {
        kind: parent.kind.clone(),
        version: parent.version,
        entries: parent.entries.clone(),
        parents: vec![parent_cid.to_string()],
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MergeStrategy {
    /// Prefer left (base) on conflict.
    Ours,
    /// Prefer right (incoming) on conflict.
    Theirs,
    /// Error if any key exists in both with different CIDs.
    Strict,
}

/// Merge `incoming` into `base`. Parent list becomes `[base_cid, other_cid]`.
pub fn merge_workspaces(
    base: &WorkspaceRoot,
    base_cid: &str,
    incoming: &WorkspaceRoot,
    other_cid: &str,
    strategy: MergeStrategy,
) -> Result<WorkspaceRoot, WorkspaceError> {
    let mut entries = base.entries.clone();
    for (k, v_in) in &incoming.entries {
        match entries.get(k) {
            None => {
                entries.insert(k.clone(), v_in.clone());
            }
            Some(v_base) if v_base == v_in => {}
            Some(_) => match strategy {
                MergeStrategy::Ours => {}
                MergeStrategy::Theirs => {
                    entries.insert(k.clone(), v_in.clone());
                }
                MergeStrategy::Strict => {
                    return Err(WorkspaceError::MergeConflict(k.clone()));
                }
            },
        }
    }
    Ok(WorkspaceRoot {
        kind: base.kind.clone(),
        version: base.version,
        entries,
        parents: vec![base_cid.to_string(), other_cid.to_string()],
    })
}

/// Insert or replace a named slot and return an updated manifest (not yet stored).
pub fn put_entry(mut w: WorkspaceRoot, name: String, cid: String) -> WorkspaceRoot {
    w.entries.insert(name, cid);
    w
}

