CREATE TABLE IF NOT EXISTS cache (
    name TEXT NOT NULL,
    confidence REAL NOT NULL,
    hash TEXT PRIMARY KEY,
    errors TEXT,
    warnings TEXT
);
