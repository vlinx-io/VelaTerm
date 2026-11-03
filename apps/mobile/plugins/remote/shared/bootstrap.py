"""Prepare a loopback VelaTerm service over an authenticated SSH connection.

Uses only Python's standard library. Installation verifies minisign using OpenSSL
Ed25519 and hashlib BLAKE2b, matching the desktop's pinned updater key.
"""
import base64
import hashlib
import json
import os
from pathlib import Path
import platform
import secrets
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request

VERSION = "0.2.14"
# Same two-line minisign public key as src-tauri/src/server_supply.rs.
PUBLIC_KEY_FILE = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDg3QkY3RjE5NjU5NEIzN0YKUldSL3M1UmxHWCsvaDF4bkdReURmL2FLV2ZRbDU1V0xyRGV0dHZwQnBibWxPU3pGdXRjc2x4eCsK"
ROOT = Path.home() / ".velaterm"
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def fetch(url, limit=256 * 1024 * 1024):
    if not url.startswith("https://dl.velaterm.com/"):
        raise ValueError("Untrusted artifact URL")
    with urllib.request.urlopen(url, timeout=120) as response:
        if not response.url.startswith("https://dl.velaterm.com/"):
            raise ValueError("Untrusted download redirect")
        body = response.read(limit + 1)
        if len(body) > limit:
            raise ValueError("Artifact exceeds size limit")
        return body


def verify(data, entry):
    if hashlib.sha256(data).hexdigest() != entry["sha256"]:
        raise ValueError("Artifact SHA-256 mismatch")
    key = base64.b64decode(base64.b64decode(PUBLIC_KEY_FILE).decode().splitlines()[1])
    lines = base64.b64decode(entry["signature"]).decode().splitlines()
    signature = base64.b64decode(lines[1])
    if len(key) != 42 or len(signature) != 74 or signature[2:10] != key[2:10]:
        raise ValueError("Invalid minisign key or signature")
    if signature[:2] == b"ED":
        message = hashlib.blake2b(data).digest()
    elif signature[:2] == b"Ed":
        message = data
    else:
        raise ValueError("Unsupported minisign algorithm")
    openssl = shutil.which("openssl")
    if not openssl:
        raise RuntimeError("Automatic installation needs OpenSSL with Ed25519 support; install it or use an existing service port")
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        # SubjectPublicKeyInfo for Ed25519 (RFC 8410).
        (directory / "key.der").write_bytes(bytes.fromhex("302a300506032b6570032100") + key[10:])
        (directory / "signature").write_bytes(signature[10:])
        (directory / "message").write_bytes(message)
        result = subprocess.run([openssl, "pkeyutl", "-verify", "-pubin", "-keyform", "DER", "-inkey", str(directory / "key.der"), "-rawin", "-in", str(directory / "message"), "-sigfile", str(directory / "signature")], capture_output=True)
        if result.returncode:
            raise ValueError("Artifact signature verification failed; OpenSSL must support Ed25519")


def alive(record):
    try:
        port = int(record["port"])
        if not 0 < port < 65536 or not isinstance(record["password"], str):
            return False
        request = urllib.request.Request("http://127.0.0.1:%d/api/login" % port, data=json.dumps({"password": record["password"]}).encode(), headers={"Content-Type": "application/json"})
        with HTTP.open(request, timeout=3) as response:
            return bool(json.load(response).get("token"))
    except (OSError, ValueError, KeyError):
        return False


def prepare(allow_install):
    system = platform.system()
    if system == "Darwin":
        desktop = Path.home() / "Library/Application Support/io.vlinx.vlxterm.release"
        oskey = "darwin"
    elif system == "Windows":
        desktop = Path(os.environ["APPDATA"]) / "io.vlinx.vlxterm.release"
        oskey = "windows"
    elif system == "Linux":
        desktop = Path(os.environ.get("XDG_DATA_HOME", str(Path.home() / ".local/share"))) / "io.vlinx.vlxterm.release"
        oskey = "linux"
    else:
        raise RuntimeError("Unsupported remote operating system: " + system)
    for path in [desktop / "vlx-local-link.json", ROOT / "run.json", ROOT / "mobile-run.json"]:
        try:
            record = json.loads(path.read_text())
            if alive(record):
                return {"port": record["port"], "password": record["password"], "source": "existing"}
        except (OSError, ValueError):
            pass
    if not allow_install:
        raise RuntimeError("No running VelaTerm service; enable service preparation or specify an existing service port")
    arch = {"arm64": "aarch64", "aarch64": "aarch64", "AMD64": "x86_64", "x86_64": "x86_64"}.get(platform.machine())
    if not arch:
        raise RuntimeError("Unsupported remote CPU architecture")
    manifest = json.loads(fetch("https://dl.velaterm.com/server/%s/server-manifest.json" % VERSION, 1024 * 1024))
    if manifest.get("version") != VERSION:
        raise ValueError("Server manifest version mismatch")
    entry = manifest["platforms"][oskey + "-" + arch]
    binary = ROOT / "versions" / VERSION / ("vela-server.exe" if system == "Windows" else "vela-server")
    data = binary.read_bytes() if binary.exists() else fetch(entry["url"])
    try:
        verify(data, entry)
    except ValueError:
        if not binary.exists():
            raise
        data = fetch(entry["url"])
        verify(data, entry)
    binary.parent.mkdir(parents=True, exist_ok=True)
    if not binary.exists() or binary.read_bytes() != data:
        staging = binary.with_name(".mobile-" + secrets.token_hex(8))
        staging.write_bytes(data)
        staging.chmod(0o700)
        staging.replace(binary)
    # Preserve a previously allocated service port whenever it is still available.
    config_path = ROOT / "mobile-service.json"
    try:
        config = json.loads(config_path.read_text())
        port = int(config["port"])
        if not 10000 <= port <= 49151:
            raise ValueError("invalid port")
    except (OSError, ValueError, KeyError):
        port = 0
    for _ in range(100):
        port = port or secrets.randbelow(39152) + 10000
        with socket.socket() as listener:
            try:
                listener.bind(("127.0.0.1", port))
                break
            except OSError:
                port = 0
    if not port:
        raise RuntimeError("No free service port")
    config_path.write_text(json.dumps({"port": port}))
    config_path.chmod(0o600)
    password = secrets.token_hex(32)
    data_dir = desktop if (desktop / "vlx-term.db").exists() else ROOT / "data"
    data_dir.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, VELA_SERVE_PASSWORD=password)
    options = {"creationflags": subprocess.DETACHED_PROCESS | subprocess.CREATE_NEW_PROCESS_GROUP | subprocess.CREATE_BREAKAWAY_FROM_JOB} if system == "Windows" else {"start_new_session": True}
    with (ROOT / "mobile-server.log").open("ab") as log:
        process = subprocess.Popen([str(binary), "--serve", "--local-http", "--port", str(port), "--data-dir", str(data_dir), "--mirror", "0"], stdin=subprocess.DEVNULL, stdout=log, stderr=log, env=environment, **options)
    record = {"pid": process.pid, "port": port, "password": password, "version": VERSION}
    state = ROOT / "mobile-run.json"
    state.write_text(json.dumps(record))
    state.chmod(0o600)
    for _ in range(30):
        if alive(record):
            return {"port": port, "password": password, "source": "prepared"}
        if process.poll() is not None:
            raise RuntimeError("VelaTerm service exited; inspect ~/.velaterm/mobile-server.log")
        time.sleep(1)
    raise RuntimeError("VelaTerm service did not become ready; inspect ~/.velaterm/mobile-server.log")


if __name__ == "__main__":
    try:
        os.umask(0o077)
        ROOT.mkdir(mode=0o700, exist_ok=True)
        lock = ROOT / "mobile-bootstrap.lock"
        try:
            lock.mkdir()
        except FileExistsError:
            raise RuntimeError("Another service preparation is in progress; if it was interrupted, remove ~/.velaterm/mobile-bootstrap.lock")
        try:
            print(json.dumps(prepare("--install" in sys.argv)))
        finally:
            lock.rmdir()
    except Exception as error:
        print(json.dumps({"error": str(error)}))
        # The SSH wire protocol reports errors in JSON, including on Windows shells.
        # Both native clients must reject an error object before opening a tunnel.
