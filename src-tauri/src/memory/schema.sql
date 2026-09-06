CREATE TABLE IF NOT EXISTS memory_entries (
 id TEXT PRIMARY KEY,
 title TEXT NOT NULL,
 title_key TEXT NOT NULL UNIQUE,
 summary TEXT NOT NULL,
 content TEXT NOT NULL,
 tags TEXT NOT NULL,
 related TEXT NOT NULL,
 sources TEXT NOT NULL,
 version INTEGER NOT NULL,
 created_at INTEGER NOT NULL,
 updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS memory_versions (
 entry_id TEXT NOT NULL REFERENCES memory_entries(id) ON DELETE CASCADE,
 version INTEGER NOT NULL,
 snapshot TEXT NOT NULL,
 author TEXT NOT NULL,
 created_at INTEGER NOT NULL,
 PRIMARY KEY(entry_id, version)
);
-- Source snapshots survive deletion of the original session and its project.
CREATE TABLE IF NOT EXISTS memory_sources (
 id TEXT PRIMARY KEY,
 session_id TEXT NOT NULL,
 session_name TEXT NOT NULL,
 kind TEXT NOT NULL,
 agent_session_id TEXT NOT NULL,
 digest TEXT NOT NULL,
 content TEXT NOT NULL,
 created_at INTEGER NOT NULL,
 UNIQUE(session_id, digest)
);
CREATE TABLE IF NOT EXISTS memory_jobs (
 id TEXT PRIMARY KEY,
 source_id TEXT NOT NULL REFERENCES memory_sources(id),
 agent TEXT NOT NULL,
 model TEXT NOT NULL,
 status TEXT NOT NULL,
 stage TEXT NOT NULL,
 progress INTEGER NOT NULL DEFAULT 0,
 total INTEGER NOT NULL DEFAULT 0,
 error TEXT NOT NULL DEFAULT '',
 entries TEXT NOT NULL DEFAULT '[]',
 owner_pid INTEGER NOT NULL,
 created_at INTEGER NOT NULL,
 updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS memory_jobs_source ON memory_jobs(source_id, status);
CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(title, summary, content, tags, tokenize='trigram');
CREATE TRIGGER IF NOT EXISTS memory_fts_insert AFTER INSERT ON memory_entries BEGIN
 INSERT INTO memory_fts(rowid,title,summary,content,tags) VALUES(new.rowid,new.title,new.summary,new.content,new.tags);
END;
CREATE TRIGGER IF NOT EXISTS memory_fts_update AFTER UPDATE ON memory_entries BEGIN
 DELETE FROM memory_fts WHERE rowid=old.rowid;
 INSERT INTO memory_fts(rowid,title,summary,content,tags) VALUES(new.rowid,new.title,new.summary,new.content,new.tags);
END;
CREATE TRIGGER IF NOT EXISTS memory_fts_delete AFTER DELETE ON memory_entries BEGIN
 DELETE FROM memory_fts WHERE rowid=old.rowid;
END;
-- Serialize compilers across windows/processes that share this database.
CREATE UNIQUE INDEX IF NOT EXISTS memory_one_running ON memory_jobs((1)) WHERE status='running';
