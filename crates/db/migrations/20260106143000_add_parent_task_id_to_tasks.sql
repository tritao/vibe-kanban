-- Add parent_task_id for true subtasks (task hierarchy).
-- Keep existing parent_workspace_id (lineage/provenance) unchanged.

PRAGMA foreign_keys = ON;

ALTER TABLE tasks ADD COLUMN parent_task_id BLOB REFERENCES tasks(id);

CREATE INDEX IF NOT EXISTS idx_tasks_parent_task_id ON tasks(parent_task_id);

