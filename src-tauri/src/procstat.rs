//! Sample CPU and memory for a PID and its complete descendant tree for the Info panel's Resources section.
//!
//! Agent PTYs launch a shell whose Claude process and commands are descendants. Measuring only the
//! root is nearly zero, so subtree totals represent the session's actual resource use.
//!
//! All platforms share a persistent `sysinfo` sampler using native in-process APIs (`/proc`,
//! libproc/sysctl, or Windows APIs). This replaces the previous non-Windows `ps -axo` subprocess,
//! which scanned the entire process table every two seconds.
//!
//! CPU percentage is an increment between refreshes and requires a persistent `System`; a newly
//! created one often reports zero and has a roughly 200 ms minimum interval. A process-wide
//! `Mutex<System>` retains snapshots, reconstructs descendants by ppid, and sums CPU/memory. CPU
//! warms up over one or two samples, while memory is an accurate instantaneous snapshot immediately.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcStats {
    /// Sum of subtree CPU percentages; 100% is one core and totals may exceed it.
    pub cpu: f64,
    /// Sum of subtree resident memory (RSS) in bytes.
    pub rss_bytes: u64,
}

/// Process-wide persistent sampler retaining the previous snapshot for CPU deltas.
fn sampler() -> &'static Mutex<System> {
    static SYS: OnceLock<Mutex<System>> = OnceLock::new();
    SYS.get_or_init(|| Mutex::new(System::new()))
}

/// List `pid` and every descendant. A caller that waits for a session to stop writing must collect
/// this before the kill, because the parent links disappear with the processes.
pub fn subtree_pids(pid: u32) -> Vec<u32> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let root = Pid::from_u32(pid);
    if sys.process(root).is_none() {
        return Vec::new();
    }
    let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for (p, process) in sys.processes() {
        if let Some(parent) = process.parent() {
            children.entry(parent).or_default().push(*p);
        }
    }
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(cur) = stack.pop() {
        if found.contains(&cur) {
            continue; // Defensive cycle guard; valid process trees are acyclic.
        }
        found.push(cur);
        if let Some(kids) = children.get(&cur) {
            stack.extend(kids.iter().copied());
        }
    }
    found.iter().map(|pid| pid.as_u32()).collect()
}

/// Report which of `pids` are still running; a zombie counts as exited because only its reap remains.
/// A local `System` keeps the shared sampler's CPU deltas intact.
pub fn running_pids(pids: &[u32]) -> Vec<u32> {
    let lookup: Vec<Pid> = pids.iter().map(|pid| Pid::from_u32(*pid)).collect();
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::Some(&lookup),
        true,
        ProcessRefreshKind::nothing(),
    );
    lookup
        .iter()
        .filter(|pid| {
            sys.process(**pid)
                .is_some_and(|process| process.status() != sysinfo::ProcessStatus::Zombie)
        })
        .map(|pid| pid.as_u32())
        .collect()
}

/// Sum CPU percentage and RSS for the process tree rooted at `pid`. Refresh the persistent `System`,
/// reconstruct parent-child relationships by ppid, and traverse from the root. Return None if absent.
pub fn subtree_stats(pid: u32) -> Option<ProcStats> {
    let mut sys = sampler().lock().ok()?;
    // Refresh only CPU and memory; refresh_all would unnecessarily scan disks and networks. The true
    // flag removes exited processes from the snapshot.
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cpu().with_memory(),
    );

    let root = Pid::from_u32(pid);
    // Return None when the target process does not exist.
    if sys.process(root).is_none() {
        return None;
    }

    // Map ppid to child PIDs, copying Pid values to release the System borrow.
    let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for (p, process) in sys.processes() {
        if let Some(parent) = process.parent() {
            children.entry(parent).or_default().push(*p);
        }
    }

    // Traverse from the root and accumulate CPU percentage and RSS.
    let mut cpu = 0.0_f64;
    let mut rss_bytes: u64 = 0;
    let mut stack = vec![root];
    let mut seen: HashMap<Pid, ()> = HashMap::new();
    while let Some(cur) = stack.pop() {
        if seen.insert(cur, ()).is_some() {
            continue; // Defensive cycle guard; valid process trees are acyclic.
        }
        if let Some(process) = sys.process(cur) {
            cpu += process.cpu_usage() as f64; // 100% equals one core.
            rss_bytes += process.memory(); // sysinfo 0.33 reports memory in bytes.
        }
        if let Some(kids) = children.get(&cur) {
            stack.extend(kids.iter().copied());
        }
    }

    Some(ProcStats { cpu, rss_bytes })
}
