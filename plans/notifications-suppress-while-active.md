# Suppress Notifications While User Is Active In UI

## Problem

Vibe Kanban sends OS notifications / sounds for:

- Task completion (`crates/services/src/services/container.rs:164`)
- Tool approval requests (`crates/services/src/services/approvals/executor_approvals.rs:60`)

Even when the operator is already focused in the web UI, actively working on (and watching) the
attempt. This creates noisy, redundant alerts.

## Goal

Do not send notifications for an attempt when a user is *actively viewing it* in the UI.

“Active” should mean:

- The browser tab is visible (`document.visibilityState === 'visible'`)
- The browser window is focused (i.e. the user has not alt-tabbed away)
- The currently selected attempt/workspace matches the event’s workspace
- Presence is recent (heartbeat TTL)

## Non-goals

- Full, durable, multi-device presence tracking stored in the DB.
- Perfect correctness across server restarts (best-effort is fine).
- Fine-grained “I’m reading the diff vs chat panel” activity.

## Proposed design (high level)

1. Frontend periodically reports **presence** to the backend for the currently viewed workspace.
2. Backend keeps a short-lived, in-memory map of recent presence by user/session.
3. Before sending a notification for a workspace event, backend checks:
   - “Is any active presence for this user currently focused+visible on this workspace?”
   - If yes, **suppress** notification (sound + push).

This keeps notifications useful for background work, without bothering the operator while they’re
already watching.

## UX / behavior rules

- If the tab is visible + focused and the operator is on the same attempt:
  - **No OS notification**
  - **No sound**
- If the operator is on a different attempt:
  - Notify as today
- If the operator is on the same attempt but the tab is not focused/visible (e.g. user alt-tabbed away):
  - Notify as today
- If multiple users are in the org:
  - Only suppress for the user whose config/notifications would be used (single-user local setup is
    the common case; multi-user hosting can evolve later).

## Data model (in-memory)

In the server process:

```rust
struct PresenceState {
  user_id: Option<Uuid>,         // if available from auth
  session_id: Option<Uuid>,      // optional
  workspace_id: Option<Uuid>,    // the attempt being viewed
  visible: bool,
  focused: bool,
  last_seen: DateTime<Utc>,
}
```

Storage:

- `DashMap<ClientKey, PresenceState>` where `ClientKey` is derived from auth identity:
  - Preferred: user id
  - Fallback: API token id / session cookie id

TTL:

- Consider presence “active” if `now - last_seen <= 30s`.

## API changes

### Endpoint

Add:

- `POST /api/presence`

Payload:

```ts
type PresenceUpdate = {
  workspace_id: string | null;
  visible: boolean;
  focused: boolean;
};
```

Response:

- `200 OK` (no body required)

Auth:

- Use the same auth mechanism as other UI API calls.
- If unauthenticated, either:
  - accept but key presence by remote IP + user-agent (low security), or
  - reject and keep current behavior (simpler and safer).

## Backend integration points

### 1) Task completion notifications

Current:

- `crates/services/src/services/container.rs:210` calls `notification_service().notify(...)`

Change:

- Wrap with a predicate:
  - `if should_notify_for_workspace(workspace_id, NotificationKind::TaskCompleted) { notify }`

### 2) Approval-needed notifications

Current:

- `crates/services/src/services/approvals/executor_approvals.rs:61` always notifies

Change:

- Resolve the workspace_id for `execution_process_id` (load context or join from DB once).
- Gate notification with the same predicate.

### 3) Config toggle (optional but recommended)

Add a config flag:

- `notifications.quiet_when_active: boolean` (default `true`)

If false, do not suppress.

## Frontend implementation

### Presence reporter hook

Implement `frontend/src/hooks/usePresenceReporter.ts`:

Responsibilities:

- Determine current workspace/attempt id from router state (task attempt page) and pass `null`
  elsewhere.
- Track `document.visibilityState` changes.
- Track window focus/blur (or `document.hasFocus()`).
- Send presence updates:
  - immediately on state changes (workspace id / focus / visibility)
  - plus a heartbeat interval (e.g. every 15s) while `visible && focused`
- Throttle to avoid spam (e.g. don’t send more frequently than 2s).

### Where to mount

- Mount in the task/attempt page component so it naturally reports the selected attempt.
- Optionally mount globally and report `workspace_id = null` when not on an attempt.

## Edge cases

- Multiple tabs:
  - Only the focused tab will report `focused=true`.
  - Background tabs will report `focused=false`, so notifications won’t be suppressed.
- Network loss:
  - Presence TTL expires; notifications resume as today.
- Long-running tasks:
  - Heartbeat keeps presence fresh.

## Testing plan

Backend:

- Unit test `should_suppress_notification(workspace_id, client_key, now)` with TTL and flags.
- Integration-ish test for `/api/presence` updating the in-memory store.

Frontend:

- Minimal unit test for hook behavior (optional).
- Manual verification:
  1) Start an attempt and stay on it → when it completes, no OS notification.
  2) Switch to another tab/app → completion triggers notification.
  3) Approval requested while viewing attempt → no approval sound; when away → approval sound.

## Rollout

1. Implement backend presence store + endpoint.
2. Add frontend reporter for attempt pages.
3. Gate notifications for task completion.
4. Gate notifications for approvals.
5. Add config toggle if desired.
