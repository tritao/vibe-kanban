# Split API Server vs Worker/Runner (Option A: DB-Only Streaming)

## Problem

Today the backend process mixes two roles:

1) **API server**: HTTP + WebSocket endpoints for the UI.
2) **Runner**: starts and owns long-running executions (Codex sessions, scripts), plus in-memory log
   stores used for live streaming.

As a result:

- Restarting the backend drops all WS connections and can also interrupt/kills in-flight runs.
- In dev, the `/api/*/ws` proxy path adds extra instability (ECONNRESET), and reconnection behavior
  can look like “stuck loading”.
- There is no clean way to restart the API server independently of ongoing runs.

## Goal

Make the API server safely restartable **without losing running Codex sessions**, by moving the
“runner” responsibilities to a separate worker service that keeps running across API restarts.

## Scope (Option A)

We start with **DB-only streaming**:

- Worker writes all logs to SQLite continuously.
- API server streams logs to clients by **tailing the DB** (polling).
- We keep the option to add **Option B: Worker pub/sub log bus** later (low latency, less DB load).

## Non-goals

- Horizontal scaling across multiple machines (SQLite remains local).
- Guaranteed “reattach to OS-level stdout pipes” after a runner crash (Option A streams from DB).
- Large refactors of the frontend protocol (keep existing endpoints and JSON Patch formats).

## Current state (relevant pieces)

- `execution_processes` is the canonical “execution timeline”.
- `execution_process_logs` stores JSONL `LogMsg` lines (stdout/stderr/json_patch/finished).
- Live log WS endpoints (`/api/execution-processes/:id/*-logs/ws`) currently depend on in-memory
  `MsgStore` when available and fall back to full replay from DB (not tail).

## Proposed architecture

### Services

- **API server (`server` bin)**:
  - Owns HTTP routes and WS routes.
  - Creates “execution requests” in the DB (queued items).
  - Streams state/logs/diffs by reading from DB and worktree filesystem.
  - Does **not** spawn Codex/session runner processes.

- **Worker/runner (`worker` bin)**:
  - Polls DB for queued execution requests.
  - Claims and executes them (Codex sessions, setup scripts, dev server scripts, etc).
  - Writes logs to DB as it runs (stdout/stderr + normalized JsonPatch lines).
  - Updates execution status + exit codes in DB.
  - Supports cancel/kill via DB flags.

### Shared state

- SQLite (`dev_assets/db.sqlite` via `utils::assets::asset_dir()`).
- Shared worktree paths on disk (both processes must have access).

## Data model changes

### 1) Execution “queue” + “claim” fields

Add durable scheduling fields to `execution_processes`:

- `status`: add `queued` (recommended) OR introduce `run_state` separate from exit status.
- `claimed_by` (TEXT, nullable): worker identifier.
- `claimed_at` (DATETIME, nullable)
- `heartbeat_at` (DATETIME, nullable)
- `cancel_requested` (BOOLEAN, default 0)

Indexes:

- `(status, claimed_at)` to find unclaimed queued processes.
- `(claimed_by, heartbeat_at)` for stale detection.

### 2) Log tail cursor (optional, recommended)

To efficiently tail logs:

- Add `id INTEGER PRIMARY KEY AUTOINCREMENT` to `execution_process_logs`,
  plus index `(execution_id, id)`.

If we avoid schema change, we can use `(inserted_at, rowid)` as a cursor, but adding `id` is
cleaner and more deterministic.

## Backend implementation plan

### Milestone 1: Introduce `worker` binary + DB queue primitives

1) Add a new Rust binary:
   - Preferred: `crates/server/src/bin/worker.rs` (shares `DeploymentImpl`).
   - Alternative: a new crate `crates/worker` if dependency boundaries matter.

2) Add DB methods in `crates/db/src/models/execution_process.rs`:
   - `enqueue(...)` creates a `queued` execution record with `executor_action` + metadata.
   - `claim_next(worker_id)` atomically claims the next queued process.
   - `heartbeat(execution_id, worker_id)` updates `heartbeat_at`.
   - `request_cancel(execution_id)` sets `cancel_requested=1`.

3) Ensure the existing UI “execution process stream” (`/api/execution-processes/stream/ws`)
   includes queued processes and claim/status transitions.

### Milestone 2: Move execution spawning to worker

1) Identify all code paths that start executions today:
   - “start coding agent”
   - setup scripts / cleanup scripts
   - dev server scripts
   - any other long-running actions

2) Update API routes/services to:
   - Create an execution record as `queued`.
   - Return immediately (or return the new execution id).

3) Worker loop:
   - Poll/claim queued processes.
   - Ensure worktree exists (reuse existing worktree/container code).
   - Run the action using the same executor implementations.
   - Stream logs into DB continuously (see Milestone 3).
   - Update status to `running` → `completed/failed/killed` with exit codes.

### Milestone 3: DB-only log streaming (tailing) for WS endpoints

Goal: the API server should be able to stream logs live **without** having any in-memory `MsgStore`.

1) Add `ExecutionProcessLogs::tail(...)` in `crates/db/src/models/execution_process_logs.rs`:
   - Inputs: `execution_id`, `cursor`, `limit`.
   - Output: next chunk(s) of JSONL and an updated cursor.

2) Update WS routes:
   - `/api/execution-processes/:id/raw-logs/ws`
   - `/api/execution-processes/:id/normalized-logs/ws`

   Behavior:
   - On connect: replay from DB from cursor=0.
   - Then poll DB for new rows and emit patches as they arrive.
   - Stop when the execution process status is terminal and no more rows arrive.

3) Normalized logs:
   - Worker should ensure normalized JsonPatch `LogMsg::JsonPatch(...)` lines are written into
     `execution_process_logs` during execution (current behavior already supports mixed LogMsg).
   - API WS endpoint filters DB rows to only JsonPatch lines.

### Milestone 4: Cancel/stop semantics

1) API “stop execution” endpoint:
   - Set `cancel_requested=1` in DB.
   - Optionally update status to `killed` only once worker confirms termination.

2) Worker:
   - Periodically checks `cancel_requested`.
   - If set, stops the underlying process/session and marks status as `killed`.

### Milestone 5: Restart safety + observability

1) Remove “kill all running executions on API shutdown” for normal operation.
   - Keep an explicit admin/manual “stop all” action if needed.

2) Worker startup recovery:
   - If a process is `running` but the worker doesn’t own it (or heartbeat is stale), mark it
     `failed` with a clear error message, or attempt to recover if we track PID (optional).

3) Add minimal metrics/logging:
   - worker: claims/sec, running count, failures, cancel count, heartbeat lag.
   - API: WS connection counts, log tail poll lag.

## Frontend impact (minimal)

- Keep the same endpoints and patch shapes.
- Biggest behavioral change is that live logs should continue even if the API server restarts
  (after the UI reconnects and resumes from DB).

## Local dev workflow

- Run API server: `cargo run --bin server`
- Run worker: `cargo run --bin worker`
- Optional: add a `pnpm run dev:split` helper that starts both.

## Rollout strategy

1) Land schema + worker binary behind a config flag:
   - `USE_WORKER_QUEUE=1` makes API enqueue instead of spawn.
2) Validate correctness for:
   - start/stop runs
   - live log streaming while running
   - API restart mid-run (UI reconnects, run continues)
3) Default-enable queue mode locally, then in remote deployments.

## Future (Option B): Worker pub/sub log bus

Once Option A is stable:

- Add a worker-owned local pub/sub channel (unix socket or localhost) for low-latency fan-out.
- API server subscribes for active executions and forwards to WS clients.
- DB remains the durability layer; pub/sub is only for “live tail” performance.

