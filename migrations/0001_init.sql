-- code-grapher schema v1
CREATE TABLE IF NOT EXISTS repositories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    git_url TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    lang TEXT,
    hash TEXT,
    mtime INTEGER NOT NULL DEFAULT 0,
    loc INTEGER NOT NULL DEFAULT 0,
    parsed_at TEXT,
    UNIQUE(repo_id, path)
);

CREATE TABLE IF NOT EXISTS symbols (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    name TEXT NOT NULL,
    qual_path TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    parent_id INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
    is_pub INTEGER NOT NULL DEFAULT 0,
    UNIQUE(repo_id, qual_path)
);

CREATE TABLE IF NOT EXISTS call_edges (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    caller_id INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    callee_id INTEGER REFERENCES symbols(id) ON DELETE SET NULL,
    callee_name TEXT NOT NULL,
    kind TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'ast',
    confidence REAL NOT NULL DEFAULT 1.0,
    evidence TEXT,
    task_id INTEGER,
    UNIQUE(caller_id, callee_name, kind, source)
);

CREATE TABLE IF NOT EXISTS analysis_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    type TEXT NOT NULL DEFAULT 'hidden_calls',
    target_symbol_id INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
    payload TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    attempts INTEGER NOT NULL DEFAULT 0,
    result TEXT,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(repo_id, target_symbol_id, type)
);

CREATE TABLE IF NOT EXISTS index_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id INTEGER NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT,
    trigger TEXT NOT NULL DEFAULT 'manual',
    files_changed INTEGER NOT NULL DEFAULT 0,
    symbols INTEGER NOT NULL DEFAULT 0,
    edges INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER,
    status TEXT NOT NULL DEFAULT 'running'
);

CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);
CREATE INDEX IF NOT EXISTS idx_edges_caller ON call_edges(caller_id);
CREATE INDEX IF NOT EXISTS idx_edges_callee ON call_edges(callee_id);
CREATE INDEX IF NOT EXISTS idx_tasks_status ON analysis_tasks(status);
CREATE INDEX IF NOT EXISTS idx_runs_repo ON index_runs(repo_id);
