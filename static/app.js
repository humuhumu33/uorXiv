const out = document.getElementById("out");
/** Last workspace root CID (uorx-d-…), for “Use last”. */
let lastWorkspaceCid = "";

function log(obj) {
  const text = typeof obj === "string" ? obj : JSON.stringify(obj, null, 2);
  out.textContent = text;
}

function setWorkspace(cid) {
  if (cid) {
    document.getElementById("workspaceCid").value = cid;
    lastWorkspaceCid = cid;
  }
}

function rememberWorkspaceCid(data) {
  if (data && typeof data.cid === "string") {
    setWorkspace(data.cid);
  }
}

async function api(method, path, opts = {}) {
  const r = await fetch(path, {
    method,
    headers: opts.headers,
    body: opts.body,
  });
  const text = await r.text();
  if (!r.ok) {
    throw new Error(`${r.status} ${text}`);
  }
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

document.getElementById("btnUseLast").onclick = () => {
  if (lastWorkspaceCid) setWorkspace(lastWorkspaceCid);
  log({ used_workspace_cid: lastWorkspaceCid || "(none yet)" });
};

document.getElementById("btnUploadBlob").onclick = async () => {
  const f = document.getElementById("blobFile").files[0];
  if (!f) return log("Pick a file first.");
  try {
    const buf = await f.arrayBuffer();
    const r = await fetch("/api/blobs", {
      method: "POST",
      headers: { "Content-Type": "application/octet-stream" },
      body: buf,
    });
    const text = await r.text();
    if (!r.ok) throw new Error(text);
    const data = JSON.parse(text);
    document.getElementById("entryCid").value = data.cid;
    log(data);
  } catch (e) {
    log(String(e));
  }
};

document.getElementById("btnNewWs").onclick = async () => {
  try {
    const data = await api("POST", "/api/workspaces");
    rememberWorkspaceCid(data);
    log(data);
  } catch (e) {
    log(String(e));
  }
};

document.getElementById("btnShowWs").onclick = async () => {
  const cid = document.getElementById("workspaceCid").value.trim();
  if (!cid) return log("Set workspace CID.");
  try {
    log(await api("GET", "/api/workspaces/" + encodeURIComponent(cid)));
  } catch (e) {
    log(String(e));
  }
};

document.getElementById("btnPutEntry").onclick = async () => {
  const cid = document.getElementById("workspaceCid").value.trim();
  const name = document.getElementById("entryName").value.trim();
  const target_cid = document.getElementById("entryCid").value.trim();
  if (!cid || !name || !target_cid) return log("workspace CID, name, and target CID required.");
  try {
    const data = await api("POST", "/api/workspaces/" + encodeURIComponent(cid) + "/entries", {
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name, target_cid }),
    });
    rememberWorkspaceCid(data);
    log(data);
  } catch (e) {
    log(String(e));
  }
};

document.getElementById("btnFork").onclick = async () => {
  const cid = document.getElementById("workspaceCid").value.trim();
  if (!cid) return log("Set workspace CID.");
  try {
    const data = await api("POST", "/api/workspaces/" + encodeURIComponent(cid) + "/fork");
    rememberWorkspaceCid(data);
    log(data);
  } catch (e) {
    log(String(e));
  }
};

document.getElementById("btnMerge").onclick = async () => {
  const base = document.getElementById("mergeBase").value.trim();
  const other = document.getElementById("mergeOther").value.trim();
  const strategy = document.getElementById("mergeStrategy").value;
  if (!base || !other) return log("merge base and other required.");
  try {
    const data = await api("POST", "/api/workspaces/merge", {
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ base, other, strategy }),
    });
    rememberWorkspaceCid(data);
    log(data);
  } catch (e) {
    log(String(e));
  }
};

document.getElementById("btnPublish").onclick = async () => {
  const cid = document.getElementById("workspaceCid").value.trim();
  if (!cid) return log("Set workspace CID.");
  try {
    log(await api("POST", "/api/workspaces/" + encodeURIComponent(cid) + "/publish"));
  } catch (e) {
    log(String(e));
  }
};

document.getElementById("btnDag").onclick = async () => {
  const cid = document.getElementById("dagCid").value.trim();
  if (!cid) return log("Set DAG CID.");
  try {
    log(await api("GET", "/api/dags/" + encodeURIComponent(cid)));
  } catch (e) {
    log(String(e));
  }
};

document.getElementById("btnRun").onclick = async () => {
  const f = document.getElementById("wasmFile").files[0];
  if (!f) return log("Pick a .wasm file.");
  const fuel = document.getElementById("runFuel").value;
  const argsRaw = document.getElementById("runArgs").value.trim();
  const inputsRaw = document.getElementById("runInputs").value.trim();
  try {
    JSON.parse(argsRaw);
    JSON.parse(inputsRaw);
  } catch {
    return log("args and input_cids must be valid JSON arrays.");
  }
  const fd = new FormData();
  fd.append("wasm", f, f.name);
  fd.append("fuel", fuel);
  fd.append("args", argsRaw);
  fd.append("input_cids", inputsRaw);
  try {
    const r = await fetch("/api/run", { method: "POST", body: fd });
    const text = await r.text();
    if (!r.ok) throw new Error(text);
    const data = JSON.parse(text);
    document.getElementById("dagCid").value = data.run_cid;
    log(data);
  } catch (e) {
    log(String(e));
  }
};

log("Ready. Create a workspace or upload a blob to begin.");
