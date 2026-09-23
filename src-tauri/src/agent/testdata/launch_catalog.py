#!/usr/bin/env python3
"""Local catalogue protocol fixture. Never accepts a prompt or contacts a provider."""
import json
import os
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

base = Path(__file__).resolve().parent
kind = Path(sys.argv[0]).stem
with (base / "calls.jsonl").open("a") as log:
    log.write(json.dumps({"kind": kind, "cwd": os.getcwd(), "args": sys.argv[1:]}) + "\n")

if "--version" in sys.argv:
    print("2.1.200")
    sys.exit(0)

if "serve" in sys.argv:
    port = int(sys.argv[sys.argv.index("--port") + 1])

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            value = {"healthy": True}
            if self.path.split("?", 1)[0] == "/provider":
                value = {"connected": ["fixture"], "default": {"fixture": "deep"}, "all": [
                    {"id": "fixture", "name": "Fixture", "models": {
                        "deep": {"name": "Deep", "variants": {"high": {}, "low": {}, "custom": {}}},
                        "plain": {"name": "Plain", "variants": {}}}},
                    {"id": "disconnected", "models": {"hidden": {"variants": {"bad": {}}}}},
                ]}
            body = json.dumps(value).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args):
            pass

    HTTPServer(("127.0.0.1", port), Handler).serve_forever()
else:
    for line in sys.stdin:
        request = json.loads(line)
        if "method" in request:
            method = request["method"]
            result = {}
            if method == "model/list":
                result = {"data": [{"id": "gpt-5.6-sol", "displayName": "GPT-5.6-Sol",
                                    "supportedReasoningEfforts": [{"reasoningEffort": level} for level in ["low", "medium", "high", "xhigh", "max", "ultra"]]},
                                   {"id": "quick", "supportedReasoningEfforts": [{"reasoningEffort": "low"}]}]}
            elif method not in ("initialize", "initialized"):
                raise RuntimeError("Catalogue fixture received a non-catalogue method")
            if "id" in request:
                print(json.dumps({"id": request["id"], "result": result}), flush=True)
        elif request.get("type") == "control_request":
            # The Claude model probe: initialize and list_models both carry the models array.
            models = [
                {"value": "fixture-claude-model", "resolvedModel": "fixture-claude-model", "displayName": "Fixture Claude",
                 "description": "Fixture model", "supportedEffortLevels": ["low", "high"]},
                {"value": "fixture-claude-model-b", "displayName": "Fixture Claude B", "supportedEffortLevels": ["low"]},
            ]
            subtype = request["request"].get("subtype")
            if subtype not in ("initialize", "list_models"):
                raise RuntimeError("Catalogue fixture received a non-catalogue control request")
            print(json.dumps({"type": "control_response", "response": {"subtype": "success", "request_id": request["request_id"],
                              "response": {"commands": [], "models": models}}}), flush=True)
        else:
            command = request["type"]
            if command not in ("get_available_models", "negotiate_protocol"):
                raise RuntimeError("Catalogue fixture received a non-catalogue command")
            models = [
                {"id": "shared", "provider": "provider-a", "name": "Shared A", "reasoning": True,
                 "thinkingLevelMap": {"xhigh": "highest"}, "thinking": {"efforts": ["low", "high", "max"]}},
                {"id": "shared", "provider": "provider-b", "name": "Shared B", "reasoning": False},
            ]
            print(json.dumps({"type": "response", "id": request["id"], "command": command,
                              "success": True, "data": {"models": models}}), flush=True)
