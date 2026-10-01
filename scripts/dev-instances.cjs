#!/usr/bin/env node
//
// dev-instances.cjs — Inspect and precisely stop development instances.
//
//   node dev-instances.cjs ls            List running instances and remove stale manifests.
//   node dev-instances.cjs stop <label>  Stop one labeled instance, including its process tree and ports.
//   node dev-instances.cjs stop all      Stop every instance.
//
// Development launchers write `.dev-data/instances/<label>.json` through dev-lib.sh. Each manifest
// contains `{ label, mode, pid, vitePort?, backendPort?, dataDir?, url?, bind?, startedAt }`, where pid
// is the launcher and process-tree root (bind is dev:web's VLX_DEV_BIND mode: lan or loopback). Stopping recursively terminates descendants, clears recorded
// ports as a fallback, and removes the manifest without affecting other instances.
//
// See the development modes and ports design document for background.

const fs = require("node:fs");
const path = require("node:path");
const cp = require("node:child_process");

const DIR = path.join(__dirname, "..", ".dev-data", "instances");

/** Read every valid manifest and attach its source path as `_file`. */
function loadAll() {
  if (!fs.existsSync(DIR)) return [];
  return fs
    .readdirSync(DIR)
    .filter((f) => f.endsWith(".json"))
    .map((f) => {
      const file = path.join(DIR, f);
      try {
        const e = JSON.parse(fs.readFileSync(file, "utf8"));
        e._file = file;
        return e;
      } catch {
        return null;
      }
    })
    .filter(Boolean);
}

/** Probe process liveness with signal 0; EPERM still means the process exists. */
function alive(pid) {
  if (!pid) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (e) {
    return e.code === "EPERM";
  }
}

/** Recursively collect descendant PIDs using `pgrep -P`. */
function descendants(pid) {
  const out = [];
  const stack = [pid];
  while (stack.length) {
    const p = stack.pop();
    let kids = [];
    try {
      kids = cp
        .execSync(`pgrep -P ${p}`, { stdio: ["ignore", "pipe", "ignore"] })
        .toString()
        .split(/\s+/)
        .filter(Boolean)
        .map(Number);
    } catch {
      kids = [];
    }
    for (const k of kids) {
      out.push(k);
      stack.push(k);
    }
  }
  return out;
}

/** Terminate processes holding a port when they have escaped the launcher tree. */
function killPort(port) {
  if (!port) return;
  let pids = [];
  try {
    pids = cp
      .execSync(`lsof -ti tcp:${port}`, { stdio: ["ignore", "pipe", "ignore"] })
      .toString()
      .split(/\s+/)
      .filter(Boolean)
      .map(Number);
  } catch {
    pids = [];
  }
  for (const p of pids) {
    try {
      process.kill(p, "SIGTERM");
    } catch {
      /* Already exited. */
    }
  }
}

/** Remove stale manifests and print live instances. */
function cmdLs() {
  const all = loadAll();
  const live = [];
  for (const e of all) {
    if (alive(e.pid)) {
      live.push(e);
    } else {
      try {
        fs.rmSync(e._file);
      } catch {
        /* Ignore cleanup failure. */
      }
    }
  }
  if (live.length === 0) {
    console.log("No dev instances are running.");
    return;
  }
  const rows = live.map((e) => ({
    label: e.label || "?",
    mode: e.mode || "?",
    pid: String(e.pid || "?"),
    vite: e.vitePort ? String(e.vitePort) : "-",
    backend: e.backendPort ? String(e.backendPort) : "-",
    url: e.url || "-",
    started: e.startedAt || "-",
  }));
  const cols = ["label", "mode", "pid", "vite", "backend", "url", "started"];
  const head = { label: "LABEL", mode: "MODE", pid: "PID", vite: "VITE", backend: "BACKEND", url: "URL", started: "STARTED" };
  const width = {};
  for (const c of cols) {
    width[c] = Math.max(head[c].length, ...rows.map((r) => r[c].length));
  }
  const fmt = (r) => cols.map((c) => r[c].padEnd(width[c])).join("  ");
  console.log(fmt(head));
  for (const r of rows) console.log(fmt(r));
}

/** Stop an exact label or every instance. */
function cmdStop(target) {
  if (!target) {
    console.error("usage: pnpm dev:stop <label|all> (run pnpm dev:ls to list running instances)");
    process.exit(1);
  }
  const all = loadAll();
  const hits = target === "all" ? all : all.filter((e) => e.label === target);
  if (hits.length === 0) {
    console.error(`No dev instance named ${target}. Run pnpm dev:ls to list them.`);
    process.exit(1);
  }
  for (const e of hits) {
    const tree = e.pid ? descendants(e.pid) : [];
    // Terminate deepest descendants first, then the launcher so its cleanup trap can still run.
    for (const p of tree.reverse()) {
      try {
        process.kill(p, "SIGTERM");
      } catch {
        /* Already exited. */
      }
    }
    if (e.pid) {
      try {
        process.kill(e.pid, "SIGTERM");
      } catch {
        /* Already exited. */
      }
    }
    // Clear recorded ports in case a process escaped the launcher's tree.
    killPort(e.vitePort);
    killPort(e.backendPort);
    try {
      fs.rmSync(e._file);
    } catch {
      /* The launcher's trap may already have removed it. */
    }
    console.log(`Stopped dev instance ${e.label} (${e.mode}, pid ${e.pid})`);
  }
}

const [cmd, arg] = process.argv.slice(2);
if (cmd === "ls") {
  cmdLs();
} else if (cmd === "stop") {
  cmdStop(arg);
} else {
  console.error("usage: node dev-instances.cjs <ls|stop> [label]");
  process.exit(1);
}
