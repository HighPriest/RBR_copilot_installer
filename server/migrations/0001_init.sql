-- Auth schema. Applied idempotently at startup (task S5).
-- Raw tokens are never stored: token_hash = lowercase hex sha256(raw).
-- ACCESS MODEL: no tiers. A credential holds grants; each grant names one
-- package directory it may list/download. The `public` directory needs no
-- credential and never appears in grants.

CREATE TABLE IF NOT EXISTS tokens (
    token_hash TEXT PRIMARY KEY,
    label      TEXT,
    created_at INTEGER NOT NULL,
    expires_at INTEGER,           -- unix seconds; NULL = never
    revoked    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS grants (
    token_hash TEXT NOT NULL REFERENCES tokens(token_hash) ON DELETE CASCADE,
    directory  TEXT NOT NULL,     -- matches PackageInfo.directory ([a-z0-9_-]+)
    PRIMARY KEY (token_hash, directory)
);
