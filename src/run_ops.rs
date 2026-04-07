//! Persist Wasm run outputs as blobs + DAG [`crate::ipld::RunRecord`].

use crate::ipld::{RunRecord, TraceMetricsRecord, RUN_KIND};
use crate::sandbox::{run_wasm_wasi, WasmRunLimits, WasmRunOutcome};
use crate::store::ContentStore;
use anyhow::{anyhow, Context, Result};

/// Execute WASI module bytes and store stdout/stderr blobs plus a run manifest; returns ([`RunRecord`], run_manifest_cid).
pub fn persist_wasm_run(
    store: &dyn ContentStore,
    wasm_cid: &str,
    wasm_bytes: &[u8],
    input_cids: Vec<String>,
    program_args: &[String],
    limits: &WasmRunLimits,
) -> Result<(RunRecord, String)> {
    let out = run_wasm_wasi(wasm_bytes, program_args, limits).context("wasm run")?;
    persist_outcome(store, wasm_cid, input_cids, out)
}

pub fn persist_outcome(
    store: &dyn ContentStore,
    wasm_cid: &str,
    input_cids: Vec<String>,
    out: WasmRunOutcome,
) -> Result<(RunRecord, String)> {
    let stdout_cid = store.add_blob(&out.stdout)?;
    let stderr_cid = store.add_blob(&out.stderr)?;

    let metrics = crate::uor_impl::SandboxTraceMetrics {
        step_count: 1,
        total_ring_distance: out.stdout.len() as u64,
        total_hamming_distance: out.stderr.len() as u64,
    };

    let record = RunRecord {
        kind: RUN_KIND.to_string(),
        wasm_cid: wasm_cid.to_string(),
        input_cids,
        exit_code: out.exit_code,
        stdout_cid,
        stderr_cid,
        trace_cid: String::new(),
        metrics: TraceMetricsRecord {
            step_count: metrics.step_count,
            total_ring_distance: metrics.total_ring_distance,
            total_hamming_distance: metrics.total_hamming_distance,
        },
    };
    record.validate().map_err(|e| anyhow!("invalid run record: {}", e))?;
    let v = record.to_json_value()?;
    let run_cid = store.dag_put_json(&v)?;
    Ok((record, run_cid))
}
