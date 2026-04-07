//! Content-addressed storage: IPFS CLI adapter and in-memory test double.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum IpfsCliError {
    #[error("ipfs command failed: {0}")]
    Command(String),
    #[error("ipfs returned non-utf8 output")]
    Utf8,
}

/// Minimal store for blobs and DAG JSON (Kubo `ipfs` CLI).
pub trait ContentStore {
    fn add_blob(&self, data: &[u8]) -> Result<String>;
    fn cat_blob(&self, cid: &str) -> Result<Vec<u8>>;
    fn dag_put_json(&self, value: &Value) -> Result<String>;
    fn dag_get_json(&self, cid: &str) -> Result<Value>;
    fn pin(&self, cid: &str) -> Result<()>;
}

/// Invoke `ipfs` from `PATH` (or set `IPFS_BIN` to the executable path).
#[derive(Debug, Clone)]
pub struct IpfsCliStore {
    pub program: String,
}

impl Default for IpfsCliStore {
    fn default() -> Self {
        Self {
            program: std::env::var("IPFS_BIN").unwrap_or_else(|_| "ipfs".to_string()),
        }
    }
}

impl IpfsCliStore {
    fn run<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let out = Command::new(&self.program)
            .args(args)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output()
            .with_context(|| format!("failed to spawn {}", self.program))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(anyhow!(IpfsCliError::Command(err.trim().to_string())));
        }
        String::from_utf8(out.stdout)
            .map(|s| s.trim().to_string())
            .map_err(|_| anyhow!(IpfsCliError::Utf8))
    }

    fn run_stdin<I, S>(&self, args: I, stdin: &[u8]) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut child = Command::new(&self.program)
            .args(args)
            .stdin(Stdio::piped())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to spawn {}", self.program))?;
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(stdin)
            .context("write stdin")?;
        let out = child.wait_with_output().context("wait ipfs")?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(anyhow!(IpfsCliError::Command(err.trim().to_string())));
        }
        String::from_utf8(out.stdout)
            .map(|s| s.trim().to_string())
            .map_err(|_| anyhow!(IpfsCliError::Utf8))
    }
}

impl ContentStore for IpfsCliStore {
    fn add_blob(&self, data: &[u8]) -> Result<String> {
        self.run_stdin(["add", "-Q", "--stdin-name", "blob"], data)
    }

    fn cat_blob(&self, cid: &str) -> Result<Vec<u8>> {
        let out = Command::new(&self.program)
            .args(["cat", cid])
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output()
            .with_context(|| format!("failed to spawn {}", self.program))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(anyhow!(IpfsCliError::Command(err.trim().to_string())));
        }
        Ok(out.stdout)
    }

    fn dag_put_json(&self, value: &Value) -> Result<String> {
        let json = serde_json::to_vec(value).context("serialize dag json")?;
        self.run_stdin(["dag", "put", "--input-enc", "json"], &json)
    }

    fn dag_get_json(&self, cid: &str) -> Result<Value> {
        let s = self.run(["dag", "get", cid])?;
        serde_json::from_str(&s).context("parse dag get json")
    }

    fn pin(&self, cid: &str) -> Result<()> {
        self.run(["pin", "add", cid])?;
        Ok(())
    }
}

#[derive(Debug, Default)]
struct MemoryStoreInner {
    blobs: HashMap<String, Vec<u8>>,
    dags: HashMap<String, Value>,
    next: u64,
}

/// In-memory store for unit tests (synthetic CIDs).
#[derive(Debug, Default)]
pub struct MemoryStore {
    inner: Mutex<MemoryStoreInner>,
}

impl MemoryStore {
    fn alloc_cid(inner: &mut MemoryStoreInner, prefix: &str) -> String {
        inner.next += 1;
        format!("{}{:016x}", prefix, inner.next)
    }
}

impl ContentStore for MemoryStore {
    fn add_blob(&self, data: &[u8]) -> Result<String> {
        let mut g = self.inner.lock().expect("memory store lock");
        let cid = Self::alloc_cid(&mut g, "bafymemb");
        g.blobs.insert(cid.clone(), data.to_vec());
        Ok(cid)
    }

    fn cat_blob(&self, cid: &str) -> Result<Vec<u8>> {
        let g = self.inner.lock().expect("memory store lock");
        g.blobs
            .get(cid)
            .cloned()
            .ok_or_else(|| anyhow!("unknown blob cid {}", cid))
    }

    fn dag_put_json(&self, value: &Value) -> Result<String> {
        let mut g = self.inner.lock().expect("memory store lock");
        let cid = Self::alloc_cid(&mut g, "bafymemd");
        g.dags.insert(cid.clone(), value.clone());
        Ok(cid)
    }

    fn dag_get_json(&self, cid: &str) -> Result<Value> {
        let g = self.inner.lock().expect("memory store lock");
        g.dags
            .get(cid)
            .cloned()
            .ok_or_else(|| anyhow!("unknown dag cid {}", cid))
    }

    fn pin(&self, _cid: &str) -> Result<()> {
        Ok(())
    }
}
