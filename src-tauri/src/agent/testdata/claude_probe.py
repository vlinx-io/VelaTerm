#!/usr/bin/env python3
"""Fake Claude CLI for the model-catalogue probe. Never accepts a prompt or contacts a provider.

The test writes this file with CONFIG filled in: `version` is what `--version` prints, `models` is the
list both control responses carry, `mode` is one of `ok`, `garbage`, `error`, `exit3`, and `sleep` delays
the answers by that many seconds, and `grandchild` starts a `sleep` child first and records its pid in
`grandchild.pid`. Every invocation is logged to `calls.jsonl` beside the script, with the
process id, the working directory, the arguments and the VelaTerm/Claude environment keys it received.
"""
import json
import os
import sys
import time
from pathlib import Path

CONFIG = json.loads(r'''__CONFIG__''')

base = Path(__file__).resolve().parent
env_keys = sorted(k for k in os.environ if k.startswith("VLX_") or k.startswith("CLAUDE"))
with (base / "calls.jsonl").open("a") as log:
    log.write(json.dumps({"pid": os.getpid(), "cwd": os.getcwd(), "args": sys.argv[1:], "env": env_keys}) + "\n")

if "--version" in sys.argv:
    print(CONFIG.get("version", "2.1.280") + " (Claude Code)")
    sys.exit(0)

mode = CONFIG.get("mode", "ok")
if mode == "exit3":
    sys.exit(3)
if CONFIG.get("grandchild"):
    # A process the CLI starts, standing in for a hook or MCP server; the probe's timeout must end it too.
    import subprocess
    grandchild = subprocess.Popen(["sleep", "30"])
    (base / "grandchild.pid").write_text(str(grandchild.pid))
if CONFIG.get("sleep"):
    time.sleep(float(CONFIG["sleep"]))
# A real CLI prints hook and system lines before answering; the parser must step over them.
print(json.dumps({"type": "system", "subtype": "hook_started", "hook_name": "SessionStart", "hook_id": "h1"}), flush=True)
print("garbage that is not json", flush=True)
if mode == "garbage":
    print(json.dumps({"type": "system", "subtype": "hook_response", "hook_id": "h1"}), flush=True)
    sys.exit(0)
models = CONFIG.get("models", [])
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    request = json.loads(line)
    if request.get("type") != "control_request":
        continue
    subtype = request["request"].get("subtype")
    request_id = request["request_id"]
    if mode == "error":
        response = {"subtype": "error", "request_id": request_id, "error": "Not logged in"}
    elif subtype == "initialize":
        response = {"subtype": "success", "request_id": request_id, "response": {"commands": [], "models": models}}
    elif subtype == "list_models":
        response = {"subtype": "success", "request_id": request_id, "response": {"models": models}}
    else:
        continue
    print(json.dumps({"type": "control_response", "response": response}), flush=True)
