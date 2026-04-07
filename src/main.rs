//! CLI for UOR-rooted workspace (IPFS) and Wasm runs.

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use std::fs;
use std::path::PathBuf;
use uor_xiv::sandbox::WasmRunLimits;
use uor_xiv::store::{ContentStore, IpfsCliStore, LocalFsStore, MemoryStore};
use uor_xiv::workspace::{fork_workspace, load_workspace, merge_workspaces, put_entry, save_workspace, MergeStrategy};
use uor_xiv::{persist_wasm_run, CidAddress, WorkspaceRoot};

#[derive(Parser)]
#[command(name = "uor-xiv", about = "UOR-rooted shared workspace (IPFS + Wasm)")]
struct Cli {
    /// Persistent local store directory (SHA-256 addressed blobs/dags; no `ipfs` daemon).
    #[arg(long, global = true, value_name = "DIR")]
    store: Option<PathBuf>,
    /// Ephemeral in-process store only (cannot chain commands across runs).
    #[arg(long, global = true)]
    memory: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add a file as a raw IPFS block; print CID.
    Add {
        path: PathBuf,
    },
    /// Print raw bytes of a blob CID to stdout.
    Cat {
        cid: String,
    },
    /// Put JSON file as DAG-JSON; print CID.
    DagPut {
        path: PathBuf,
    },
    /// Pretty-print a DAG JSON node by CID.
    DagShow {
        cid: String,
    },
    /// Create an empty workspace manifest.
    WorkspaceNew,
    /// Show workspace JSON for a root CID.
    WorkspaceShow {
        cid: String,
    },
    /// Add or replace a named entry; prints new workspace root CID.
    WorkspacePutEntry {
        #[arg(long)]
        workspace: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        cid: String,
    },
    /// Fork workspace (same entries, parent = source CID).
    WorkspaceFork {
        cid: String,
    },
    /// Merge `other` into `base`; prints merged root CID.
    WorkspaceMerge {
        #[arg(long)]
        base: String,
        #[arg(long)]
        other: String,
        #[arg(long, value_enum, default_value_t = MergeStrategyArg::Strict)]
        strategy: MergeStrategyArg,
    },
    /// Pin a CID locally (`ipfs pin add`).
    WorkspacePublish {
        cid: String,
    },
    /// Fetch Wasm by CID, run under WASI preview1, store stdout/stderr + run record DAG.
    Run {
        #[arg(long)]
        wasm_cid: String,
        #[arg(long, default_value_t = 10_000_000_u64)]
        fuel: u64,
        #[arg(long, action = clap::ArgAction::Append)]
        input_cid: Vec<String>,
        /// Guest argv (first is often the wasm module name).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        guest_args: Vec<String>,
    },
    /// Print UOR address view of a CID (glyph/digest/quantum).
    AddressShow {
        cid: String,
    },
}

#[derive(Clone, Copy, Default, ValueEnum)]
enum MergeStrategyArg {
    #[default]
    Strict,
    Ours,
    Theirs,
}

impl From<MergeStrategyArg> for MergeStrategy {
    fn from(a: MergeStrategyArg) -> Self {
        match a {
            MergeStrategyArg::Strict => MergeStrategy::Strict,
            MergeStrategyArg::Ours => MergeStrategy::Ours,
            MergeStrategyArg::Theirs => MergeStrategy::Theirs,
        }
    }
}

enum StoreKind {
    Ipfs(IpfsCliStore),
    Mem(MemoryStore),
    Local(LocalFsStore),
}

impl StoreKind {
    fn as_store(&self) -> &dyn ContentStore {
        match self {
            StoreKind::Ipfs(s) => s,
            StoreKind::Mem(s) => s,
            StoreKind::Local(s) => s,
        }
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = match (&cli.store, cli.memory) {
        (Some(dir), _) => StoreKind::Local(LocalFsStore::open(dir)?),
        (None, true) => StoreKind::Mem(MemoryStore::default()),
        (None, false) => StoreKind::Ipfs(IpfsCliStore::default()),
    };
    let s = store.as_store();

    match cli.command {
        Commands::Add { path } => {
            let data = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            let cid = s.add_blob(&data)?;
            println!("{}", cid);
        }
        Commands::Cat { cid } => {
            let data = s.cat_blob(&cid)?;
            std::io::Write::write_all(&mut std::io::stdout(), &data)?;
        }
        Commands::DagPut { path } => {
            let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            let v: serde_json::Value = serde_json::from_str(&text).context("parse json")?;
            let cid = s.dag_put_json(&v)?;
            println!("{}", cid);
        }
        Commands::DagShow { cid } => {
            let v = s.dag_get_json(&cid)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        Commands::WorkspaceNew => {
            let w = WorkspaceRoot::empty();
            let cid = save_workspace(s, &w)?;
            println!("{}", cid);
        }
        Commands::WorkspaceShow { cid } => {
            let w = load_workspace(s, &cid)?;
            println!("{}", serde_json::to_string_pretty(&w)?);
        }
        Commands::WorkspacePutEntry {
            workspace,
            name,
            cid: entry_cid,
        } => {
            let mut w = load_workspace(s, &workspace)?;
            w = put_entry(w, name, entry_cid);
            let new_cid = save_workspace(s, &w)?;
            println!("{}", new_cid);
        }
        Commands::WorkspaceFork { cid } => {
            let w = load_workspace(s, &cid)?;
            let forked = fork_workspace(&w, &cid);
            let new_cid = save_workspace(s, &forked)?;
            println!("{}", new_cid);
        }
        Commands::WorkspaceMerge {
            base,
            other,
            strategy,
        } => {
            let w_base = load_workspace(s, &base)?;
            let w_other = load_workspace(s, &other)?;
            let merged = merge_workspaces(&w_base, &base, &w_other, &other, strategy.into())
                .map_err(|e| anyhow!("{}", e))?;
            let new_cid = save_workspace(s, &merged)?;
            println!("{}", new_cid);
        }
        Commands::WorkspacePublish { cid } => {
            s.pin(&cid)?;
            println!("pinned {}", cid);
        }
        Commands::Run {
            wasm_cid,
            fuel,
            input_cid,
            guest_args,
        } => {
            let wasm_bytes = s.cat_blob(&wasm_cid).context("fetch wasm blob")?;
            let limits = WasmRunLimits {
                fuel,
                ..Default::default()
            };
            let (_record, run_cid) = persist_wasm_run(
                s,
                &wasm_cid,
                &wasm_bytes,
                input_cid,
                &guest_args,
                &limits,
            )?;
            println!("{}", run_cid);
        }
        Commands::AddressShow { cid } => {
            let a = CidAddress::from_cid(&cid);
            use uor_foundation::kernel::address::Address;
            println!("glyph: {}", a.glyph());
            println!("length: {}", a.length());
            println!("addresses: {}", a.addresses());
            println!("digest: {}", a.digest());
            println!("digest_algorithm: {}", a.digest_algorithm());
            println!("quantum: {}", a.quantum());
        }
    }

    Ok(())
}
