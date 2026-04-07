//! UOR-rooted shared workspace: IPLD manifests, IPFS storage, Wasm execution.

pub mod ipld;
pub mod primitives;
pub mod run_ops;
pub mod sandbox;
pub mod store;
pub mod uor_impl;
pub mod workspace;

pub use ipld::{RunRecord, TraceMetricsRecord, WorkspaceRoot, WORKSPACE_KIND, RUN_KIND};
pub use primitives::AppPrimitives;
pub use sandbox::{run_wasm_wasi, WasmRunLimits, WasmRunOutcome};
pub use store::{ContentStore, IpfsCliError, IpfsCliStore, MemoryStore};
pub use uor_impl::CidAddress;
pub use run_ops::{persist_outcome, persist_wasm_run};
pub use workspace::WorkspaceError;
