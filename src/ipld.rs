//! IPLD v0 workspace and run records (JSON on the wire for `ipfs dag put`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// `kind` field for [`WorkspaceRoot`].
pub const WORKSPACE_KIND: &str = "uor-xiv/workspace@v0";
/// `kind` field for [`RunRecord`].
pub const RUN_KIND: &str = "uor-xiv/run@v0";

/// Current manifest schema version (bump when fields change).
pub const SCHEMA_VERSION: u32 = 0;

/// Workspace manifest: named slots map to content CIDs; `parents` form the DAG for fork/merge.
///
/// **Invariants (v0):**
/// - Every value in `entries` MUST be a valid IPFS CID string pointing at raw bytes or another DAG node.
/// - `parents` MUST list prior workspace root CIDs (empty for genesis).
/// - `kind` MUST be [`WORKSPACE_KIND`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceRoot {
    pub kind: String,
    pub version: u32,
    /// Slot name → CID of blob or nested DAG.
    pub entries: BTreeMap<String, String>,
    /// Provenance: parent workspace root CIDs (merge has multiple).
    pub parents: Vec<String>,
}

impl WorkspaceRoot {
    pub fn empty() -> Self {
        Self {
            kind: WORKSPACE_KIND.to_string(),
            version: SCHEMA_VERSION,
            entries: BTreeMap::new(),
            parents: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.kind != WORKSPACE_KIND {
            return Err("workspace.kind mismatch");
        }
        if self.version != SCHEMA_VERSION {
            return Err("workspace.version unsupported");
        }
        for (name, cid) in &self.entries {
            if name.is_empty() {
                return Err("workspace.entries empty name");
            }
            if cid.is_empty() {
                return Err("workspace.entries empty cid");
            }
        }
        Ok(())
    }

    pub fn to_json_value(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }

    pub fn from_json_value(v: &serde_json::Value) -> serde_json::Result<Self> {
        serde_json::from_value(v.clone())
    }
}

/// Record of a Wasm WASI run: all large payloads are CID-referenced (stdout/stderr blobs).
///
/// **Invariants (v0):**
/// - `wasm_cid` addresses the Wasm module bytes stored on IPFS.
/// - `input_cids` lists workspace artifact CIDs passed into the runner (audit trail); the module reads them via WASI preopens if configured by the host.
/// - `stdout_cid` / `stderr_cid` MUST reference raw blobs written by this tool after the run.
/// - `trace_cid` MAY point at an optional extended trace DAG (reserved for future UOR witnesses).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunRecord {
    pub kind: String,
    pub wasm_cid: String,
    pub input_cids: Vec<String>,
    pub exit_code: i32,
    pub stdout_cid: String,
    pub stderr_cid: String,
    /// Optional CID for a richer trace / certificate DAG (empty if none).
    pub trace_cid: String,
    pub metrics: TraceMetricsRecord,
}

/// Serializable mirror of sandbox metrics (paired with [`crate::uor_impl::SandboxTraceMetrics`]).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TraceMetricsRecord {
    pub step_count: u64,
    pub total_ring_distance: u64,
    pub total_hamming_distance: u64,
}

impl RunRecord {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.kind != RUN_KIND {
            return Err("run.kind mismatch");
        }
        if self.wasm_cid.is_empty() {
            return Err("run.wasm_cid empty");
        }
        if self.stdout_cid.is_empty() || self.stderr_cid.is_empty() {
            return Err("run stdout/stderr cid empty");
        }
        Ok(())
    }

    pub fn to_json_value(&self) -> serde_json::Result<serde_json::Value> {
        serde_json::to_value(self)
    }

    pub fn from_json_value(v: &serde_json::Value) -> serde_json::Result<Self> {
        serde_json::from_value(v.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_roundtrip() {
        let mut w = WorkspaceRoot::empty();
        w.entries.insert("doc".into(), "bafyfoo".into());
        w.parents.push("bafyparent".into());
        let v = w.to_json_value().unwrap();
        let w2 = WorkspaceRoot::from_json_value(&v).unwrap();
        assert_eq!(w, w2);
        w2.validate().unwrap();
    }
}
