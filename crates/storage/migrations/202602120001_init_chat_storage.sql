CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    active_branch_id TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER
);

CREATE INDEX idx_sessions_updated_not_deleted
    ON sessions (deleted_at, updated_at DESC, id DESC);

CREATE INDEX idx_sessions_active_branch
    ON sessions (active_branch_id);

CREATE TABLE branches (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    parent_branch_id TEXT,
    created_at INTEGER NOT NULL,
    deleted_at INTEGER,
    FOREIGN KEY (session_id) REFERENCES sessions (id) ON DELETE RESTRICT,
    FOREIGN KEY (parent_branch_id) REFERENCES branches (id) ON DELETE SET NULL,
    UNIQUE (session_id, id)
);

CREATE INDEX idx_branches_session_deleted_created
    ON branches (session_id, deleted_at, created_at DESC);

CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    branch_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    deleted_at INTEGER,
    FOREIGN KEY (session_id) REFERENCES sessions (id) ON DELETE RESTRICT,
    -- Composite FK guarantees the branch belongs to the same session.
    FOREIGN KEY (session_id, branch_id) REFERENCES branches (session_id, id) ON DELETE RESTRICT,
    CHECK (role IN ('system', 'user', 'assistant')),
    UNIQUE (session_id, branch_id, seq),
    UNIQUE (session_id, id)
);

CREATE INDEX idx_messages_session_branch_seq
    ON messages (session_id, branch_id, seq);

CREATE INDEX idx_messages_session_deleted_seq
    ON messages (session_id, deleted_at, seq);

CREATE TABLE media_refs (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    message_id TEXT NOT NULL,
    uri TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    duration_ms INTEGER,
    width_px INTEGER,
    height_px INTEGER,
    sha256_hex TEXT,
    created_at INTEGER NOT NULL,
    deleted_at INTEGER,
    FOREIGN KEY (session_id) REFERENCES sessions (id) ON DELETE RESTRICT,
    FOREIGN KEY (session_id, message_id) REFERENCES messages (session_id, id) ON DELETE RESTRICT,
    UNIQUE (session_id, id)
);

CREATE INDEX idx_media_refs_session_message_deleted
    ON media_refs (session_id, message_id, deleted_at);

CREATE TABLE agent_events (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    message_id TEXT,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (session_id) REFERENCES sessions (id) ON DELETE RESTRICT,
    FOREIGN KEY (session_id, message_id) REFERENCES messages (session_id, id) ON DELETE RESTRICT,
    UNIQUE (session_id, id)
);

CREATE INDEX idx_agent_events_session_message_created
    ON agent_events (session_id, message_id, created_at);
