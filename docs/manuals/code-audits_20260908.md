# Code audits

Created: 2026-09-08

Updated: 2026-09-25 10:21

> Code audits are experimental.

A code audit is a security review of a project that runs through your local coding agent. Right-click a project in the sidebar and choose "Experimental" → "Code audit", then "New audit". Under "Local agent", choose "Codex" or "Claude"; then choose a "Model", a "Thinking effort" and the "Scope", and click "Start audit". The report page shows the scan's current phase. "Agent session" shows the agent's original tool calls, analysis and pending permission requests; answer requests there to let the audit continue. The report appears when Codex Security seals its results.

The audit runs on the machine that hosts the project and uses the selected agent's installed CLI with its existing login, configuration and permission handling ("Uses this machine's agent login and configuration. No separate model API setup is needed."). VelaTerm adds a Codex Security MCP server for the audit session only; your other MCP servers stay as they are, and VelaTerm does not call a model API itself. The CLI must be installed, signed in and within its account limits. The CLI's own configuration may select a model provider; VelaTerm does not change it.

The "Code audit" item also appears for collections. A collection without a root folder cannot be audited.

## Models and thinking effort

The model list is the same one the new-session dialog uses for that agent, and it takes the agent's default launch arguments into account, so the list matches how the audit will run (see [Model Catalog Sync](model-catalog-sync_20260909.md)). "Default model" keeps the agent's own choice. "Thinking effort" becomes available when you choose a model that offers effort levels; "Auto" keeps the agent's own choice. If the list cannot be loaded, a retry button appears. The backend checks explicit combinations, and the audit saves the selected values for its history and for later runs. A model appearing in the list does not mean your account may use it. Provider failures, including exhausted usage limits, end the audit as failed.

New audits ask for the analysis in the interface language. Protocol fields, paths, identifiers and quoted source evidence are left unchanged.

## Codex Security integration

VelaTerm bundles the public Apache-2.0 release `@openai/codex-security@0.1.26`, whose plugin manifest is version `0.1.95`; the new-audit page shows both versions. The original skills, references, schemas, scripts, runtime and license are kept under `src-tauri/resources/codex-security/`, and `UPSTREAM.json` records the package source, integrity value and file hashes.

Every new audit registers a real upstream workbench scan. The agent reads the bundled entry skill and uses the upstream MCP tools and result format. Records from the earlier, independently implemented batch workflow remain readable and are labeled "Legacy audit · This record did not use the Codex Security workflow."

Both agents use the same upstream workflow. The public TypeScript SDK drives the Codex runtime and does not offer a Claude Code executor, so VelaTerm integrates through the upstream MCP server and local workbench. Support for Claude Code is a VelaTerm adapter, not an upstream claim of official Claude support.

The runtime needs Node.js on PATH, in a version supported by the upstream package, and Python 3.10 or later; Python 3.10 also needs the `tomli` package, which Python 3.11 and later replace with the built-in `tomllib`. VelaTerm checks the bundled resources and unpacks them into its data directory. It does not install them into your Codex or Claude configuration.

## Scope and workflow

- **Whole repository** runs the upstream Standard scan against a fixed snapshot of the target and the repository's security policy.
- **Directory or file** runs Standard with an exact "Relative path". Other source files may be read to understand a call chain, without claiming that those areas received a full audit.
- **Working tree changes** runs the upstream diff skill against a fixed Git change set. It includes staged, unstaged and untracked changes that the upstream target resolver supports, and keeps baseline evidence for deleted files.

Standard reads `skills/security-scan/SKILL.md` and `references/core-scan.md`. It performs preflight checks, independent baseline and architecture analysis, source-anchored investigations, candidate checkpoints, counter-evidence review, validation and coverage reconciliation. Independent workers are used when the agent supports them; otherwise the upstream sequential fallback applies, and its limitations must be disclosed. A worker-capacity setting alone is not evidence that independent workers ran.

Working-tree audits read `skills/security-diff-scan/SKILL.md`. They fix the target, build the threat model, prepare and enumerate review items, find candidates, validate them, analyze attack paths and submit the result draft for completion. Reviewing only the current contents of changed files does not replace this flow.

The upstream workbench accepts only directories as a scope. To keep file selection working, the installed runtime applies two narrow patches whose hashes are recorded: an existing file is allowed as the scope and counts as one snapshot entry. The bundled originals are unchanged, and the installed `ADAPTER.json` records both the original and the adapted hashes. Selecting a file never turns into a scan of its whole directory.

The audit instructions forbid source edits, fixes, publication and changes to the agent's configuration. The agent's own tool permissions stay in force. These instructions are not an operating-system sandbox. Validation must state what was actually checked, and a static finding does not mean that an exploit was reproduced ("Static review only. Reported coverage does not guarantee that every vulnerability was found. Dynamic exploitation is not performed.").

## Reports, coverage and cancellation

The authoritative results are the upstream `scan-manifest.json`, `findings.json` and `coverage.json`. The upstream finalizer validates and seals them and then generates the readable report. VelaTerm imports these files and checks each finding's location and exact source lines, treating LF and CRLF line endings as equal. A valid structure and matching source lines do not by themselves prove that a finding can be exploited. If the source evidence does not match, the audit stays "Failed" and its findings are not accepted as validated. The sealed upstream report remains available, and Markdown exports put the validation failure first without rewriting the report.

Before sealing, VelaTerm checks the draft's paths, line ranges and excerpts with the same verifier used during import. If the evidence does not match, the scan stays unsealed and the agent is told which locations to correct. The Codex Security completion tool then generates and seals the report. This check never rewrites excerpts, removes findings or changes sealed results. Import repeats the check, so later source changes cannot silently turn invalid evidence into a successful result.

Coverage comes from the upstream review receipts and decisions, not from the number of files the agent claims to have read. Unresolved candidates, deferred work, exclusions and completion warnings stay visible. A sealed scan with incomplete coverage is shown as "Partial coverage". Even complete coverage does not guarantee that every vulnerability was found.

When the agent ends a turn while the scan still has work to do, VelaTerm continues the same session and scan, keeping existing findings and worker results. Permission requests stay pending ("Permission or input required"), and cancellation or a provider error stops the automatic continuation. If three turns in a row end without any change in the upstream review progress, findings or coverage, the audit stops with an incomplete-scan error and keeps its checkpoints.

The "Report" tab shows the retained findings, source evidence and coverage limits. Each finding has its own URL and shows its attack path, how it was validated and the conclusion (when supplied), remediation advice and the reported source excerpts; check the audit status before treating these excerpts as verified. The "Report artifacts" tab keeps the complete report generated upstream and the scan ID. A running scan shows the upstream package and plugin versions, the actual phase and progress, and warnings; until a sealed report exists, progress and checkpoint information are shown instead of an empty assessment. In the "Agent session", the normal message box and rewind are disabled so that an unrelated turn cannot replace the audit; permission requests remain usable.

"Cancel" stops the upstream scan and the agent. Saved checkpoints remain available when the upstream workbench can seal them, and they keep their canceled or failed status. Late results cannot revive a canceled audit. After a restart, VelaTerm reconciles the upstream state: a completed, sealed result can be imported, and an unfinished run is marked "Interrupted", with its retained files kept distinguishable from a completed result.

"Start audit" on an existing record creates a new scan of the current target with the saved agent and selections. It does not resume a stopped model turn or reuse an old verdict.

The audit history, the new-audit form, report details and the agent session each have their own URL, so they can be opened directly, refreshed and navigated with the browser's back and forward buttons. On narrow screens, the history and the details are shown as separate views.

## Exports and limits

"Export report" offers Markdown and JSON. Markdown exports contain the upstream report when one exists; otherwise they clearly identify themselves as the record of an incomplete scan. JSON exports keep all imported data together with VelaTerm's own metadata. The upstream scan folder also keeps its generated SARIF and other files. Exports can contain source excerpts and vulnerability details, so share them carefully.

This interface offers Standard and working-tree diff audits. Other bundled skills, such as deep scans, remediation, fix verification and external issue tracking, are kept as upstream resources but are not available as separate actions. An audit does not apply fixes, publish findings, create issues or open pull requests.

## Diagnostics

The backend records the audit lifecycle and upstream phase information in `runtime-*.log` in the application log directory. `VLX_LOG_DIR` sets the shared location; when it is not set, `VLX_SECURITY_LOG_DIR` sets the location for audit events. `VLX_SECURITY_LOG_LEVEL=error` limits them to failures, and `off` turns them off. Prompts and responses are described by their size and SHA-256 digest; full source code, credentials and full AI responses are not copied into the log, and raw provider messages are not kept. Custom model names are replaced by anonymous identifiers valid only in the current process. Token counts, when available, are the session's cumulative usage. The agent's conversation keeps its normal record. See [Runtime logs and privacy](runtime-diagnostics_20260909.md) for general log behavior.

For a failed audit, read the provider's explanation on the report or open "Agent session". Resolve sign-in, runtime, permission or account-limit problems before starting another audit. A connected MCP server only shows that the tools are available; it does not show that the model completed the scan.

Upstream references: [CLI overview](https://learn.chatgpt.com/docs/security/cli), [CLI reference](https://learn.chatgpt.com/docs/security/cli/reference), [public source](https://github.com/openai/codex-security).
