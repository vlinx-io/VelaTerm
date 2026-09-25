// @vitest-environment node
//
// dev-serve.test.mjs: hermetic checks of scripts/dev-serve.sh's bind-mode resolution (VLX_DEV_BIND).
//
// The script runs with VLX_DEV_DRY_RUN=1, which resolves the configuration, prints the banner plus one
// DRY_RUN line with the resolved bindings, and exits before probing ports, registering the instance, or
// starting cargo/Vite. Nothing binds a port. The LAN address lookup is pinned by PATH shims for
// `ipconfig` (macOS) and `hostname` (Linux) so the default banner can be compared line by line.
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

const here = path.dirname(fileURLToPath(import.meta.url));
const script = path.join(here, "dev-serve.sh");
const instancesDir = path.join(here, "..", ".dev-data", "instances");
const LABEL = "dev-serve-test";
const FAKE_LAN_IP = "192.0.2.10";

// The dev scripts are Bash; Windows runs them through Git Bash, which this hermetic test does not locate.
const bashUsable = process.platform !== "win32" && spawnSync("bash", ["--version"]).status === 0;

let tmp;
let dataDir;
let shimDir;

beforeAll(() => {
  if (!bashUsable) return;
  tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dev-serve-test-"));
  dataDir = path.join(tmp, "data");
  shimDir = path.join(tmp, "bin");
  fs.mkdirSync(shimDir);
  for (const [name, body] of [
    ["ipconfig", `#!/bin/sh\necho ${FAKE_LAN_IP}\n`],
    ["hostname", `#!/bin/sh\necho ${FAKE_LAN_IP}\n`],
    // Fail-safe: should the dry-run hook ever stop working, the launcher must not start a real backend or
    // Vite (which would bind 0.0.0.0 in lan mode). These shims record the attempt and fail instead.
    ["cargo", `#!/bin/sh\ntouch "${tmp}/launched"\nexit 97\n`],
    ["pnpm", `#!/bin/sh\ntouch "${tmp}/launched"\nexit 97\n`],
  ]) {
    const file = path.join(shimDir, name);
    fs.writeFileSync(file, body);
    fs.chmodSync(file, 0o755);
  }
});

afterAll(() => {
  if (tmp) fs.rmSync(tmp, { recursive: true, force: true });
});

/** Run the script in dry-run mode with a controlled environment. */
function run(extraEnv = {}) {
  const env = {
    ...process.env,
    PATH: `${shimDir}${path.delimiter}${process.env.PATH}`,
    VLX_DEV_DRY_RUN: "1",
    VLX_DEV_LABEL: LABEL,
    VLX_DEV_DATA_DIR: dataDir,
  };
  for (const key of ["VLX_DEV_BIND", "VELA_SERVE_PASSWORD", "VLX_VITE_HOST", "VLX_VITE_HMR_HOST", "TAURI_DEV_HOST"]) delete env[key];
  Object.assign(env, extraEnv);
  const res = spawnSync("bash", [script], { env, encoding: "utf8", timeout: 20000 });
  return { status: res.status, stdout: res.stdout ?? "", stderr: res.stderr ?? "" };
}

/** Parse the DRY_RUN key=value line. */
function resolved(stdout) {
  const line = stdout.split("\n").find((l) => l.startsWith("DRY_RUN "));
  expect(line).toBeDefined();
  return Object.fromEntries(
    line
      .slice("DRY_RUN ".length)
      .split(" ")
      .map((kv) => [kv.slice(0, kv.indexOf("=")), kv.slice(kv.indexOf("=") + 1)]),
  );
}

describe.skipIf(!bashUsable)("dev-serve.sh bind modes", () => {
  it("defaults to the LAN mode with today's bindings and banner when VLX_DEV_BIND is unset", () => {
    const { status, stdout } = run();
    expect(status).toBe(0);
    expect(resolved(stdout)).toEqual({
      bind: "lan",
      httpFlag: "--lan-http",
      probeHost: "0.0.0.0",
      viteHost: "0.0.0.0",
      hmrHost: FAKE_LAN_IP,
      url: `http://${FAKE_LAN_IP}:0`,
    });
    // The banner of the default mode, unchanged from before the loopback mode existed.
    const banner = stdout.slice(0, stdout.indexOf("DRY_RUN "));
    expect(banner).toBe(
      [
        "============================================================",
        ` dev · web mode (HMR)  ·  instance ${LABEL}`,
        `   URL       : http://${FAKE_LAN_IP}:0      (also reachable from other devices on this network)`,
        "   Local     : http://localhost:0",
        "   Password  : dev",
        `   Data dir  : ${dataDir}`,
        "   Ports     : Vite 0 / backend 0 (both random, so instances run in parallel)",
        `   Pair link : vlxterm://pair?host=${FAKE_LAN_IP}&port=0&password=dev&name=dev`,
        "   (type the link into Add host in the phone app, or turn it into a QR code)",
        ` Ctrl+C stops both the backend and Vite; elsewhere, use pnpm dev:stop ${LABEL}`,
        "============================================================",
        "",
      ].join("\n"),
    );
  });

  it("treats an explicit lan like unset", () => {
    expect(run({ VLX_DEV_BIND: "lan" }).stdout).toBe(run().stdout);
  });

  it("binds loopback only with VLX_DEV_BIND=loopback", () => {
    const { status, stdout } = run({ VLX_DEV_BIND: "loopback", VLX_VITE_HMR_HOST: "inherited.example" });
    expect(status).toBe(0);
    expect(resolved(stdout)).toEqual({
      bind: "loopback",
      httpFlag: "--local-http",
      probeHost: "127.0.0.1",
      viteHost: "127.0.0.1",
      // Unset on purpose, even when inherited: the HMR socket follows the page origin.
      hmrHost: "",
      url: "http://localhost:0",
    });
    expect(stdout).toContain("   URL       : http://localhost:0      (bound to 127.0.0.1, not reachable from the network)");
    expect(stdout).toContain("ssh -L 0:localhost:0 <this-host>  (not tailscale serve: local-mode rights)");
    expect(stdout).not.toContain(FAKE_LAN_IP);
    expect(stdout).not.toContain("Pair link");
  });

  it("never prints the password in loopback mode and names its source instead", () => {
    const byDefault = run({ VLX_DEV_BIND: "loopback" }).stdout;
    expect(byDefault).toContain(`   Password  : generated, stored in ${dataDir}/dev-password (mode 0600, not shown)`);
    expect(byDefault).not.toMatch(/Password\s*:\s*dev\b/);
    expect(byDefault).not.toContain("password=");

    const custom = run({ VLX_DEV_BIND: "loopback", VELA_SERVE_PASSWORD: "s3cret-pw" }).stdout;
    expect(custom).toContain("   Password  : from VELA_SERVE_PASSWORD (not shown)");
    expect(custom).not.toContain("s3cret-pw");
  });

  it("fails fast on an unknown VLX_DEV_BIND value", () => {
    const { status, stdout, stderr } = run({ VLX_DEV_BIND: "localhost" });
    expect(status).toBe(2);
    expect(stdout).toBe("");
    expect(stderr).toContain("Unknown VLX_DEV_BIND value: 'localhost' (accepted: loopback, lan; unset means lan).");
  });

  // The dry run reports variables; this one follows the real launch path in loopback mode so a launch line
  // that ignored them (a hard-coded --lan-http, a lost VLX_VITE_HOST export) would fail. cargo, pnpm, curl and
  // the browser openers are PATH shims that record what they receive and exit; the only real socket is the
  // free-port probe on 127.0.0.1. The cargo shim also snapshots the registry entry and `dev:ls` while the
  // launcher is alive.
  it("launches the backend with --local-http and Vite on 127.0.0.1, and registers the instance, in loopback mode", () => {
    const rec = path.join(tmp, "launch");
    fs.mkdirSync(rec);
    const launchLabel = `${LABEL}-launch`;
    const launchShims = path.join(tmp, "launch-bin");
    fs.mkdirSync(launchShims);
    const instances = path.join(here, "dev-instances.cjs");
    for (const [name, body] of [
      [
        "cargo",
        `#!/bin/sh\nprintf '%s\\n' "$@" > "${rec}/cargo.args"\n` +
          `printf '%s' "\${VELA_SERVE_PASSWORD-}" > "${rec}/cargo.pw"\n` +
          `cp "${instancesDir}/${launchLabel}.json" "${rec}/registry.json"\n` +
          `node "${instances}" ls > "${rec}/ls.txt"\n`,
      ],
      [
        "pnpm",
        `#!/bin/sh\nprintf '%s\\n' "$@" > "${rec}/pnpm.args"\n` +
          `printf 'host=%s hmr=%s set=%s tauri=%s\\n' "\${VLX_VITE_HOST-}" "\${VLX_VITE_HMR_HOST-}" "\${VLX_VITE_HMR_HOST+yes}" "\${TAURI_DEV_HOST+yes}" > "${rec}/pnpm.env"\n`,
      ],
      ["curl", "#!/bin/sh\nexit 0\n"],
      ["open", `#!/bin/sh\necho "$1" > "${rec}/open.url"\n`],
      ["xdg-open", `#!/bin/sh\necho "$1" > "${rec}/open.url"\n`],
    ]) {
      const file = path.join(launchShims, name);
      fs.writeFileSync(file, body);
      fs.chmodSync(file, 0o755);
    }
    const res = run({
      VLX_DEV_DRY_RUN: "",
      VLX_DEV_BIND: "loopback",
      VLX_DEV_LABEL: launchLabel,
      VLX_VITE_HMR_HOST: "inherited.example",
      TAURI_DEV_HOST: "192.168.1.50",
      VELA_SERVE_PASSWORD: "s3cret-pw",
      PATH: `${launchShims}${path.delimiter}${shimDir}${path.delimiter}${process.env.PATH}`,
    });
    expect(res.status).toBe(0);
    expect(res.stdout).not.toContain("s3cret-pw");

    const cargoArgs = fs.readFileSync(path.join(rec, "cargo.args"), "utf8").split("\n");
    expect(cargoArgs).toContain("--local-http");
    expect(cargoArgs).not.toContain("--lan-http");
    expect(cargoArgs.slice(cargoArgs.indexOf("--data-dir"), cargoArgs.indexOf("--data-dir") + 2)).toEqual(["--data-dir", dataDir]);
    // The password still reaches the backend; only the banner hides it.
    expect(fs.readFileSync(path.join(rec, "cargo.pw"), "utf8")).toBe("s3cret-pw");

    expect(fs.readFileSync(path.join(rec, "pnpm.args"), "utf8")).toBe("exec\nvite\n");
    // Neither an inherited HMR host nor an inherited TAURI_DEV_HOST reaches Vite in loopback mode.
    expect(fs.readFileSync(path.join(rec, "pnpm.env"), "utf8")).toBe("host=127.0.0.1 hmr= set= tauri=\n");

    const entry = JSON.parse(fs.readFileSync(path.join(rec, "registry.json"), "utf8"));
    expect(entry).toMatchObject({ label: launchLabel, mode: "web", bind: "loopback" });
    expect(entry.vitePort).toBeGreaterThan(0);
    expect(entry.backendPort).toBeGreaterThan(0);
    expect(entry.url).toBe(`http://localhost:${entry.vitePort}`);
    const lsRow = fs.readFileSync(path.join(rec, "ls.txt"), "utf8").split("\n").find((l) => l.startsWith(launchLabel + " "));
    expect(lsRow).toBeDefined();
    expect(lsRow).toContain(`http://localhost:${entry.vitePort}`);

    expect(fs.readFileSync(path.join(rec, "open.url"), "utf8").trim()).toBe(`http://localhost:${entry.vitePort}`);
    // The exit trap removes the registry entry again.
    expect(fs.existsSync(path.join(instancesDir, `${launchLabel}.json`))).toBe(false);
  });

  // Without VELA_SERVE_PASSWORD a loopback instance must not fall back to the well-known `dev`: it runs with
  // the local management level. A random password is generated once into a 0600 file and reused.
  it("generates and reuses a private password instead of the default in loopback mode", () => {
    const rec = path.join(tmp, "genpw");
    fs.mkdirSync(rec);
    const shims = path.join(tmp, "genpw-bin");
    fs.mkdirSync(shims);
    for (const [name, body] of [
      ["cargo", `#!/bin/sh\nprintf '%s' "\${VELA_SERVE_PASSWORD-}" > "${rec}/cargo.pw"\n`],
      ["pnpm", "#!/bin/sh\nexit 0\n"],
      ["curl", "#!/bin/sh\nexit 0\n"],
      ["open", "#!/bin/sh\nexit 0\n"],
      ["xdg-open", "#!/bin/sh\nexit 0\n"],
    ]) {
      const file = path.join(shims, name);
      fs.writeFileSync(file, body);
      fs.chmodSync(file, 0o755);
    }
    const launch = () => run({
      VLX_DEV_DRY_RUN: "",
      VLX_DEV_BIND: "loopback",
      VLX_DEV_LABEL: `${LABEL}-genpw`,
      PATH: `${shims}${path.delimiter}${shimDir}${path.delimiter}${process.env.PATH}`,
    });
    const first = launch();
    expect(first.status).toBe(0);
    const pwFile = path.join(dataDir, "dev-password");
    const generated = fs.readFileSync(pwFile, "utf8");
    expect(generated).toMatch(/^[0-9a-f]{24}$/);
    expect((fs.statSync(pwFile).mode & 0o777).toString(8)).toBe("600");
    expect(fs.readFileSync(path.join(rec, "cargo.pw"), "utf8")).toBe(generated);
    expect(first.stdout).not.toContain(generated);
    // A second start reuses the same password.
    expect(launch().status).toBe(0);
    expect(fs.readFileSync(path.join(rec, "cargo.pw"), "utf8")).toBe(generated);
    fs.rmSync(pwFile);
  });

  it("registers and starts nothing in a dry run", () => {
    run({ VLX_DEV_BIND: "loopback" });
    run();
    expect(fs.existsSync(path.join(instancesDir, `${LABEL}.json`))).toBe(false);
    expect(fs.existsSync(path.join(tmp, "launched"))).toBe(false);
  });
});
