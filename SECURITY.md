# Report A Security Issue

## Supported versions

VelaTerm is pre-1.0 and moves forward release by release. Only the most recent release receives
security fixes; a fix ships in the next release rather than as a patch to an older one. If you are
running an older build, updating is the first step.

The current version is shown in the title bar and in [the changelog](docs/changelog.md).

## Reporting a vulnerability

**Please do not open a public issue for a security problem.**

Use GitHub's private vulnerability reporting instead: go to the
[Security tab](https://github.com/vlinx-io/VelaTerm/security) of this repository and choose *Report a
vulnerability*. The report stays private between you and the maintainers until a fix is published.

A useful report includes the affected version and platform, what an attacker gains, and the shortest
sequence of steps that reproduces the problem. A proof of concept helps but is not required.

## What to expect

VelaTerm is maintained by a small team, so please allow a few working days for a first reply. We aim
to acknowledge a report within five working days, tell you whether we consider it a vulnerability and
why, and keep you informed while a fix is prepared. Once a fix ships, the release notes describe the
issue, and we are happy to credit you by the name or handle you prefer.

## Areas worth a closer look

These parts of VelaTerm handle untrusted input or cross a trust boundary, so findings there are
especially valuable:

- **Browser remote access** — the embedded web server, login and session tokens, device pairing, and
  the end-to-end encrypted channel between a browser and the desktop application.
- **SSH remote development** — host key verification, credential handling, and the provisioning of
  the remote server binary.
- **Terminal escape sequence handling** — output written by a program in a session is untrusted
  input, including OSC sequences used for notifications, titles and clipboard access.
- **Agent integration** — the hook callbacks that report agent status, and the files VelaTerm writes
  into agent configuration directories.
- **File and document handling** — path resolution for opened files, pasted images, and the built-in
  Markdown and source viewers.

## Out of scope

Reports that describe an attacker who already has local access to an unlocked machine, or who
already controls a session the user deliberately started, are generally not treated as
vulnerabilities: a terminal is designed to run whatever its user asks it to run.
