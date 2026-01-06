-- Adds a stable per-workspace+repo baseline commit used for diff streaming.
-- This prevents diffs from becoming empty after merges when the merge-base advances.
ALTER TABLE workspace_repos ADD COLUMN diff_base_oid TEXT;

