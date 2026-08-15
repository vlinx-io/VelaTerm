# Agent Landing Contract

Status: Complete

## Purpose

Define deterministic integration for disposable agent branches. Keep temporary worker history out of the parent history.

## Decisions

- The Lead writes the final Conventional Commit subject.
- `land` accepts only a direct child with a clean worktree.
- A worker branch must contain at least one commit ahead of the target.
- Landing applies the worker's net change to the parent's current branch.
- Landing does not rebase the worker or cherry-pick temporary commits.
- A conflict restores the target and returns the conflicting paths.
- Nested workers land into their immediate parent.
- `cleanup` remains separate from `land`.
- Cleanup removes only a verified worktree and its disposable branch.
- Worker history can change when its source tree and net change stay equal.

## Recovery Model

VelaTerm stores the source tree, diff fingerprint, target tree, and resulting target commit. The record also tracks an incomplete landing.

A retry resumes staged work when its tree matches the record. A retry also detects a commit created before a crash.

## Validation

The Rust library suite passes 415 tests. Eight environment-dependent tests remain ignored.

The landing test covers two worker commits, one parent commit, retry recovery, rewritten worker history, and verified cleanup.

Conflict, dirty worktree, empty branch, invalid message, and nested-child tests pass.

## Completion Recap

VelaTerm now uses `vagent land` for agent integration. The command creates one Lead-owned parent commit from the worker's net change.

Persistent metadata makes retries and cleanup deterministic without merge ancestry. Cleanup verifies the fingerprint and target commit before removal.

The orchestration skill and manual describe the new contract. Completion date: 2026-08-14.
