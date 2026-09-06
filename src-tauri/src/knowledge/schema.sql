CREATE TABLE IF NOT EXISTS knowledge_indexes (
 id TEXT PRIMARY KEY,
 project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
 root TEXT NOT NULL,
 enabled INTEGER NOT NULL DEFAULT 0,
 status TEXT NOT NULL DEFAULT 'disabled',
 error TEXT NOT NULL DEFAULT '',
 stats TEXT NOT NULL DEFAULT '{}',
 job_id TEXT NOT NULL DEFAULT '',
 updated_at INTEGER NOT NULL,
 UNIQUE(project_id, root)
);
CREATE TABLE IF NOT EXISTS knowledge_links (
 id TEXT PRIMARY KEY,
 entry_id TEXT NOT NULL REFERENCES memory_entries(id) ON DELETE CASCADE,
 index_id TEXT NOT NULL REFERENCES knowledge_indexes(id) ON DELETE CASCADE,
 file_path TEXT NOT NULL,
 symbol TEXT NOT NULL,
 kind TEXT NOT NULL,
 digest TEXT NOT NULL,
 reviewed_at INTEGER NOT NULL,
 UNIQUE(entry_id,index_id,file_path,symbol,kind)
);
CREATE INDEX IF NOT EXISTS knowledge_links_index ON knowledge_links(index_id,file_path);
CREATE TRIGGER IF NOT EXISTS knowledge_project_removed
AFTER UPDATE OF deleted_at ON projects
WHEN NEW.deleted_at IS NOT NULL
BEGIN
 DELETE FROM knowledge_indexes WHERE project_id=NEW.id;
END;
