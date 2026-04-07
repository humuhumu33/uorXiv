//! Wasmtime + WASI preview1 execution with fuel and captured stdio.

use anyhow::{anyhow, Context, Result};
use wasmtime::*;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::{pipe::MemoryOutputPipe, I32Exit, WasiCtxBuilder};

/// Resource limits for guest code.
#[derive(Debug, Clone)]
pub struct WasmRunLimits {
    pub fuel: u64,
    pub stdout_capacity: usize,
    pub stderr_capacity: usize,
}

impl Default for WasmRunLimits {
    fn default() -> Self {
        Self {
            fuel: 10_000_000,
            stdout_capacity: 256 * 1024,
            stderr_capacity: 256 * 1024,
        }
    }
}

pub struct WasmRunOutcome {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

struct RunCtx {
    wasi: WasiP1Ctx,
    stdout: MemoryOutputPipe,
    stderr: MemoryOutputPipe,
}

/// Run a WASI preview1 module (`_start`) with captured stdout/stderr.
pub fn run_wasm_wasi(wasm_bytes: &[u8], args: &[String], limits: &WasmRunLimits) -> Result<WasmRunOutcome> {
    let mut config = Config::new();
    config.consume_fuel(true);
    let engine = Engine::new(&config).context("wasm engine")?;

    let stdout = MemoryOutputPipe::new(limits.stdout_capacity);
    let stderr = MemoryOutputPipe::new(limits.stderr_capacity);

    let mut builder = WasiCtxBuilder::new();
    builder.stdout(stdout.clone()).stderr(stderr.clone());
    if !args.is_empty() {
        builder.args(args);
    }

    let wasi = builder.build_p1();
    let ctx = RunCtx {
        wasi,
        stdout,
        stderr,
    };

    let mut store = Store::new(&engine, ctx);
    store
        .set_fuel(limits.fuel)
        .map_err(|e| anyhow!("fuel: {}", e))?;

    let mut linker: Linker<RunCtx> = Linker::new(&engine);
    preview1::add_to_linker_sync(&mut linker, |c| &mut c.wasi).context("link wasi preview1")?;

    let module = Module::from_binary(&engine, wasm_bytes).context("parse wasm module")?;
    let instance = linker
        .instantiate(&mut store, &module)
        .context("instantiate")?;

    let main = instance
        .get_typed_func::<(), ()>(&mut store, "_start")
        .context("missing _start export; build with wasm32-wasi target")?;

    let exit_code = match main.call(&mut store, ()) {
        Ok(()) => 0,
        Err(e) => {
            if let Some(I32Exit(code)) = e.downcast_ref() {
                *code
            } else {
                return Err(e.context("wasm _start"));
            }
        }
    };

    let stdout = store.data().stdout.contents().to_vec();
    let stderr = store.data().stderr.contents().to_vec();

    Ok(WasmRunOutcome {
        exit_code,
        stdout,
        stderr,
    })
}
