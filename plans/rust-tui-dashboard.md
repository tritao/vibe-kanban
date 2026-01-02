# Rust TUI Dashboard (ratatui) — Detailed Implementation Plan

## Problem

Vibe Kanban is primarily a web UI. A Rust TUI would provide a fast, full-screen, keyboard-driven
dashboard for:

- browsing projects and tasks,
- quickly changing task status,
- monitoring executions and logs in real time,
- working well over SSH and on headless machines.

This plan proposes a **client-style** TUI that talks to the existing backend API and reuses the
same real-time streaming protocol the frontend uses (WebSocket + JSON Patch).

## Goals

1. **Full-screen dashboard** with multi-pane layout (Projects → Tasks → Details/Logs).
2. **Real-time updates** via existing WS streams rather than polling.
3. Support two task presentations:
   - **Board view** (kanban columns) as the default
   - **Table view** (sortable/filterable list) as a toggle
4. **Low coupling** to server internals: keep the TUI in its own crate, treat backend as an API.
5. Stable UX: deterministic keybindings, focus management, resilient reconnect behavior.

## Non-goals (initially)

- Full parity with the web UI.
- Implementing OAuth flows inside the terminal UI.
- Replacing the current `npx` entrypoint immediately (packaging can come later).

## Recommended libraries (Rust)

- UI: `ratatui` + `crossterm`
- WebSocket client: `tokio-tungstenite`
- HTTP client: `reqwest`
- JSON Patch: `json_patch` (RFC6902)
- Serialization: `serde` + `serde_json`
- CLI flags: `clap`
- Logging/errors: `tracing` (+ `tracing-subscriber`), `anyhow`/`thiserror`

## Where it lives in this repo

Add a new workspace member:

- `crates/tui/` (binary crate)

Rationale:

- Avoids pulling `crates/server` or DB/SQLx dependencies into a terminal UI client.
- Keeps build/deps and concerns clean (API client + UI only).

## Backend discovery / configuration

Mirror the backend URL resolution used by `crates/server/src/bin/mcp_task_server.rs`:

1. If `VIBE_BACKEND_URL` is set, use it.
2. Else use `HOST` (`127.0.0.1` default) and `BACKEND_PORT` or `PORT`.
3. Else read the dev port file: `utils::port_file::read_port_file("vibe-kanban")`.

Add explicit CLI overrides:

- `--backend-url http://127.0.0.1:1234`
- or `--host` + `--port`

## Backend API surface the TUI will use

### System / config

- `GET /api/info` (see `crates/server/src/routes/config.rs`)
  - exposes config, login status, profiles, environment details

### Projects

- `GET /api/projects/stream/ws` (preferred)
- `GET /api/projects` (fallback / debugging)

### Tasks

- `GET /api/tasks/stream/ws?project_id=<uuid>` (preferred)
- `GET /api/tasks?project_id=<uuid>` (fallback)

Actions:

- `PUT /api/tasks/<task_id>` with body `UpdateTask` (status moves, title/description edits)
- `POST /api/tasks` with body `CreateTask` (optional in v1)

### Task attempts (workspaces)

- `GET /api/task-attempts?task_id=<uuid>` → `Vec<Workspace>`
- `POST /api/task-attempts` (optional in v1)
- `POST /api/task-attempts/<workspace_id>/stop` (optional)

### Execution processes + logs

- `GET /api/execution-processes/stream/ws?workspace_id=<uuid>&show_soft_deleted=...`
- Logs (open only when selected):
  - `GET /api/execution-processes/<exec_id>/raw-logs/ws`
  - `GET /api/execution-processes/<exec_id>/normalized-logs/ws`
- Stop (optional in v1):
  - `POST /api/execution-processes/<exec_id>/stop`

## Streaming protocol (WebSocket + JSON Patch)

The backend sends WS messages as JSON strings shaped like `utils::log_msg::LogMsg`, plus a special
terminal message:

- Patch message: `{"JsonPatch":[...RFC6902 ops...]}` (variant serialization)
- Finished sentinel: `{"finished":true}` (special-case in `LogMsg::to_ws_message_unchecked`)

Reference:

- `crates/utils/src/log_msg.rs`
- `frontend/src/hooks/useJsonPatchWsStream.ts` (patch apply + reconnect policy)

## Data + state model (TUI)

### Treat the TUI as a client

Prefer small DTOs inside `crates/tui` rather than depending on `crates/db`:

- keeps the crate lightweight and reduces SQLx/DB transitive deps
- lets the TUI evolve independently of DB schema/internal models

### Patch store helper

Create a reusable component:

- `JsonPatchStore<T>`
  - stores a `serde_json::Value` snapshot (initially an empty container with expected keys)
  - applies RFC6902 operations via `json_patch::patch`
  - deserializes into `T` (either after each patch, or lazily on demand)

### App state

Model a single `AppState` updated by events:

- Connection state:
  - per-stream status (connected / reconnecting / error string)
- Data:
  - `projects: ProjectsState`
  - `tasks: TasksState` (scoped to selected project)
  - `attempts: Vec<Workspace>` (loaded on selected task)
  - `execs: ExecutionProcessesState` (scoped to selected workspace)
  - `logs: LogBuffer` (scoped to selected exec)
- UI:
  - `view_mode: Board|Table`
  - `focus: Projects|Tasks|Details|Logs|Modal`
  - selection: `selected_project_id`, `selected_task_id`, `selected_attempt_id`, `selected_exec_id`
  - filters/sort/search and per-pane scroll offsets

## Concurrency + event loop

### High-level approach

- UI loop renders on a tick (e.g., 30–60 FPS) and processes input/events.
- Background tasks handle IO:
  - WS streams push patch events into a channel
  - HTTP calls run on demand for actions (status updates, fetch attempts, etc.)

### Suggested primitives

- `tokio::sync::mpsc`:
  - `UiEvent` (key presses, resize)
  - `NetEvent` (patch received, stream connected/disconnected, HTTP results)
- `tokio::sync::watch`:
  - selection changes that require stream switching (selected project/workspace/exec)

### Reconnect policy

Match frontend semantics (`frontend/src/hooks/useJsonPatchWsStream.ts`):

- exponential backoff up to 8s
- do not reconnect after `{finished:true}` or clean close
- manual `r` forces reconnect

## UI/UX: layout, focus, keybindings

### Default layout

- Top bar: app name/version, backend URL, connection status, active view mode
- Left pane: projects list
- Center pane: tasks (Board or Table)
- Right pane: details (task + attempts + execution summary)
- Bottom pane: logs (for selected execution process)
- Modal overlays: help (`?`), confirmation prompts, error/toast overlay

### Focus model

Single focus at a time; `Tab` cycles:

`Projects → Tasks → Details → Logs → Projects`

### Global keybindings

- `q`: quit
- `?`: help
- `Tab` / `Shift+Tab`: focus next/prev
- `/`: search in focused pane
- `Esc`: close modal / clear search
- `r`: reconnect streams
- `t`: toggle Board/Table view

## Tasks pane: Board view (default)

### Rendering

- 4 columns: Todo / In Progress / In Review / Done
- Cancelled:
  - hidden by default (toggle via a key or tab)
  - or rendered as a separate “tab”

### Navigation

- `h/l`: move between columns
- `j/k`: move within column
- `Enter`: select task (updates Details + logs context)

### Actions

- `←/→` (or `H/L`): move task status left/right (HTTP `PUT /api/tasks/<id>`)
- `c`: set status to cancelled (with confirm)

## Tasks pane: Table view (toggle)

### Rendering

`Table` columns (initial suggestion):

- Status
- Title
- Updated
- Attempt (running/failed)
- Executor

### Interactions

- `s`: cycle sort key (updated/title/status)
- `S`: toggle sort direction
- `f`: filter by status (cycle)
- `Enter`: select task
- `←/→`: status move (same backend call as Board)

## Details pane

### Content (v1)

- Task title + status + description preview
- Attempt summary:
  - `GET /api/task-attempts?task_id=...` (loaded on task selection)
  - show latest attempts list with branch name + created/updated
- Execution summary:
  - when an attempt/workspace is selected, subscribe to
    `/api/execution-processes/stream/ws?workspace_id=...`

### Actions (v1.5+)

- `a`: create new attempt (requires repos + executor profile selection)
- `o`: open attempt in editor (if endpoint exists and is safe for TUI)
- `x`: stop attempt / stop execution (calls stop endpoints)

## Logs pane

### Content

- When `selected_exec_id` changes:
  - close previous log stream
  - open `/api/execution-processes/<exec_id>/raw-logs/ws` (or normalized)
- Render as a scrolling paragraph or list with severity styling:
  - stdout: normal
  - stderr: red

### Controls

- `End`: toggle autoscroll
- `PgUp/PgDn`: scroll history
- `l`: clear log buffer (local only)

## Implementation milestones

### Milestone 0 — Crate + UI skeleton

1. Add `crates/tui` crate and include it in workspace `Cargo.toml`.
2. Implement terminal init/restore (alternate screen + raw mode) with `crossterm`.
3. Render a static layout with placeholder panes + help modal.

Deliverable: `cargo run -p tui` shows a full-screen UI and exits cleanly.

### Milestone 1 — Backend URL resolution + `/api/info`

1. Implement backend URL detection (env/port file) mirroring MCP server bin.
2. Add simple `reqwest` client and call `GET /api/info` on startup.
3. Show connection status + loaded config summary in the top bar.

Deliverable: dashboard shows “connected” state and backend metadata.

### Milestone 2 — Projects stream (WS + JSON Patch)

1. Implement `WsJsonPatchClient`:
   - connect via `tokio-tungstenite`
   - parse incoming messages into either:
     - `JsonPatch(Vec<Operation>)`, or
     - `Finished`
2. Apply patches into `JsonPatchStore<ProjectsState>`.
3. Render projects list and allow selecting a project.

Deliverable: projects list updates live (create/delete projects reflects in TUI).

### Milestone 3 — Tasks stream + Board view

1. Start/stop the tasks stream based on `selected_project_id`.
2. Render Board view (4 columns) as the default center pane.
3. Add selection + details preview.

Deliverable: board view updates live as tasks change.

### Milestone 4 — Status updates (move cards)

1. Implement `PUT /api/tasks/<id>` calls for status transitions.
2. Add confirm prompts for destructive actions (cancel/delete if added).
3. Optimistic UI (optional): either rely on stream to update or apply local patch first.

Deliverable: moving tasks left/right works and reflects in backend.

### Milestone 5 — Table view toggle

1. Implement Table view rendering and `t` toggle.
2. Add sorting/filtering/search within table view.

Deliverable: `t` switches between board and table without losing selection.

### Milestone 6 — Attempts + execution monitoring + logs

1. On task selection, fetch attempts: `GET /api/task-attempts?task_id=...`.
2. Allow selecting an attempt/workspace in Details.
3. Subscribe to exec processes stream for the selected workspace.
4. When an exec is selected, stream logs and display in bottom pane.

Deliverable: “what’s running” + live logs are visible from the terminal.

### Milestone 7 — Polish

- Persistent search input widget (small prompt line)
- Toast notifications for errors/reconnect
- Mouse support (optional; crossterm can emit mouse events)
- Config file for TUI preferences (optional; separate from backend config)

## Testing strategy

1. **State reducer tests**
   - deterministic selection behavior when items disappear (patch removes selected id)
2. **JSON patch application tests**
   - apply a series of ops and assert final typed state matches expected
3. **Render smoke tests**
   - `ratatui` test backend: render a frame and ensure no panics (optional)

Avoid snapshot-heavy UI tests initially; focus on correctness of state + patch handling.

## Build/run instructions (developer)

- Run backend + TUI locally:
  - start backend normally (e.g., `pnpm run backend:dev:watch`)
  - run TUI: `cargo run -p tui`
- Overrides:
  - `VIBE_BACKEND_URL=http://127.0.0.1:3001 cargo run -p tui`

## Packaging / distribution (optional follow-up)

This repo already distributes platform binaries via `npx-cli` (see `npx-cli/bin/download.js`).

Two options:

1. **Ship a separate binary** (recommended):
   - name: `vibe-kanban-tui`
   - publish alongside existing binaries in the same manifest
   - add a `npx vibe-kanban tui` subcommand that launches it
2. **Embed into existing binary**:
   - add a `--tui` mode into the main server binary (not recommended: couples UI to server)

Start with local `cargo run -p tui` during development; wire packaging only after v1 is stable.
