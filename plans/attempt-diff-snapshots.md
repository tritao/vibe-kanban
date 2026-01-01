# Attempt Diff Snapshots (Post-Merge + Stable Review)

## Problem

Today the Diff panel primarily shows a “live” diff of the attempt worktree vs the target branch’s
merge-base. After a **squash merge**, the task branch is reset to the merge commit, so the merge-base
equals the new HEAD and the live diff becomes empty. This makes it hard to review “what we shipped”
immediately after merge/rebase.

## Goals

- After **Merge**, keep the attempt’s changes viewable in the Diff panel without reopening/recreating
  the attempt.
- Support multi-repo attempts: the Diff panel must correspond to the repo the user is operating on.
- Provide a path to later make **Attempt snapshot** the default “post-merge review” view.

## Non-goals

- Perfect, history-independent “all attempt changes since creation” diffs across arbitrary rebases
  without storing patch stacks.
- Replacing the existing live diff stream (it remains the best “during iteration” view).

## Proposed UX

### Diff panel controls

1. **Repo selector (shared)**
   - The Diff panel and Git toolbar must share the same repo selection.
   - If a task has one repo, hide the selector.

2. **Diff source selector**
   - `Working tree` (live): current behavior, backed by WS stream.
   - `Attempt snapshot` (stable): a stored `(base_sha, head_sha)` pair.
   - `Merged commit`: show the squash merge commit diff (`merge_sha^..merge_sha`).

3. **Default behavior**
   - While work is ongoing: default to `Working tree`.
   - After a merge exists for the selected repo: default to `Attempt snapshot` if present, else
     `Merged commit`, else `Working tree`.
   - Persist the user’s last chosen source per attempt+repo (localStorage) so it doesn’t keep
     flipping.

### What “Attempt snapshot” means

For the immediate use case (“I merged and want to see what we shipped”), the snapshot should capture:

- `snapshot_base_sha`: the merge-base between the task branch and the target branch **right before
  merge**.
- `snapshot_head_sha`: the task branch HEAD **right before merge** (pre-reset).

That snapshot represents exactly what was merged, and remains valid after merge/rebase resets the
branch pointers.

## Data model

### Option A (recommended): New table `workspace_repo_diff_snapshots`

Create a dedicated table so we don’t overload existing tables and we can keep history if desired.

Columns (minimum viable):

- `id` (UUID)
- `workspace_id` (UUID)
- `repo_id` (UUID)
- `target_branch_name` (TEXT) – for debugging / UI hints
- `base_sha` (TEXT, NOT NULL)
- `head_sha` (TEXT, NOT NULL)
- `merge_sha` (TEXT, NULL) – populate after merge if available
- `kind` (TEXT) – e.g. `pre_merge`
- `created_at`, `updated_at`

Indexes:

- `(workspace_id, repo_id, created_at DESC)` for “latest snapshot for this attempt+repo”.

### Option B: Add columns to `workspace_repos`

Add `snapshot_base_sha`, `snapshot_head_sha`, `snapshot_merge_sha`, `snapshot_updated_at` to
`workspace_repos`.

This is simpler, but makes it harder to keep multiple snapshots (e.g. multiple merges) and mixes
runtime configuration with derived artifacts.

## Backend changes

### 1) Capture snapshot on merge

In `merge_task_attempt` (server route):

- Resolve `repo`, `workspace_repo`, `worktree_path`, `target_branch`.
- Before calling `git.merge_changes(...)`:
  - `snapshot_base_sha` = `git.get_base_commit(repo.path, workspace.branch, target_branch)`
  - `snapshot_head_sha` = `git.get_head_info(worktree_path).oid` (pre-merge head)
- Perform merge (existing code).
- Record `merge_sha` (existing return value).
- Persist snapshot record (Option A: insert; Option B: update columns).
- Persist merge record (already implemented in `merges` table).

### 2) Add diff APIs for non-live sources

Add HTTP endpoints (JSON) per attempt+repo:

- `GET /api/task-attempts/:attemptId/diff/snapshot?repo_id=...`
  - Loads latest snapshot for (workspace_id, repo_id).
  - Returns structured diffs for `base_sha..head_sha`.
- `GET /api/task-attempts/:attemptId/diff/merged?repo_id=...`
  - Loads latest merge commit for (workspace_id, repo_id).
  - Returns structured diffs for `merge_sha^..merge_sha` using `DiffTarget::Commit`.

Implementation detail:

- `GitService` already supports:
  - `DiffTarget::Commit { repo_path, commit_sha }` (parent -> commit)
- Add a new target for snapshots:
  - `DiffTarget::CommitRange { repo_path, base_sha, head_sha }`
  - Implement via libgit2 diff `base_tree -> head_tree` with rename detection.

### 3) Keep WS diff stream as-is

The live stream remains:

- `GET /api/task-attempts/:attemptId/diff/ws` (WS)

This continues to provide low-latency updates during iteration.

## Frontend changes

### 1) Unify repo selection between Git toolbar and Diff panel

Today `GitOperations` uses `useAttemptRepo(attemptId)` internally.

Refactor:

- Lift repo selection to the task page (parent of both Git toolbar and Diff panel).
- Pass `selectedRepoId` + `setSelectedRepoId` to both.
- Filter diffs shown in the Diff panel to the selected repo, using the existing `repoName/...`
  prefix emitted by the diff stream.

### 2) Add `Diff source` selector to the Diff panel header

When source is:

- `Working tree`: use existing `useDiffStream(attemptId, enabled, { refreshKey })`.
- `Attempt snapshot`: fetch once from `/diff/snapshot` and render results.
- `Merged commit`: fetch once from `/diff/merged` and render results.

Loading behavior:

- Keep the existing “no flicker on stream refresh” behavior for `Working tree`.
- For fetched diffs, keep the last successful result while refetching.

### 3) Default selection logic

- If a merge exists for the selected repo:
  - Prefer `Attempt snapshot` if snapshot exists.
  - Else fallback to `Merged commit`.
  - Else fallback to `Working tree`.
- If no merge exists:
  - Default to `Working tree`.

Persist the user’s choice in localStorage:

- Key: `diffSource:{attemptId}:{repoId}`
- Values: `worktree | snapshot | merged`

## Migration & compatibility

- Add DB migration for the chosen schema option.
- Expose snapshot + merge metadata through existing “repo status” payload if needed for UI decisions
  (or add a small endpoint to query availability).
- Backfill is optional:
  - Snapshots only need to exist for merges performed after rollout.
  - For older merges, `Merged commit` still works (we already store merge commit SHAs for direct
    merges).

## Testing plan

Rust:

- Unit test `GitService::get_diffs(DiffTarget::CommitRange)` using a temp repo with two commits.
- Unit test merge snapshot capture logic (route-level test if existing infra exists, else service
  function unit test).

Frontend:

- Component test (if present) for Diff panel switching sources and retaining selection in
  localStorage.
- Manual checklist:
  - Make changes -> Diff shows in `Working tree`.
  - Rebase -> Diff updates.
  - Merge -> Diff defaults to `Attempt snapshot` (or `Merged commit`) and still shows changes.

## Rollout steps (phased)

1. **Phase 1 (unblock review):** repo-scoped diffs + `Merged commit` view.
2. **Phase 2 (stable post-merge):** capture `Attempt snapshot` at merge time + `Attempt snapshot`
   view.
3. **Phase 3 (polish):** make `Attempt snapshot` the default post-merge view and persist user
   preferences; consider keeping snapshot history per merge.

