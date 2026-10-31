//! The schema for the local store.
//!
//! The store defines its own schema. It does not read or write the databases
//! of the product being rebuilt: those use a page cipher whose key is held by
//! the application, and interoperability was never a goal. See
//! `docs/observations.md` §4.1.
//!
//! Migrations are numbered and applied in order. Each one runs exactly once
//! and is recorded in `schema_version`, so an existing database upgrades in
//! place without losing data.

/// The schema version this build expects.
pub const CURRENT_VERSION: i64 = 2;

/// Every migration, in order.
pub const MIGRATIONS: &[&str] = &[
    // Version 1: the initial schema.
    r#"
    CREATE TABLE settings (
        key        TEXT PRIMARY KEY,
        value      TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE snippets (
        id         TEXT PRIMARY KEY,
        name       TEXT NOT NULL,
        keyword    TEXT NOT NULL,
        expansion  TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    CREATE INDEX snippets_keyword ON snippets(keyword);

    CREATE TABLE quicklinks (
        id         TEXT PRIMARY KEY,
        name       TEXT NOT NULL,
        url        TEXT NOT NULL,
        created_at TEXT NOT NULL
    );

    CREATE TABLE bookmarks (
        id         TEXT PRIMARY KEY,
        name       TEXT NOT NULL,
        url        TEXT NOT NULL,
        created_at TEXT NOT NULL
    );

    CREATE TABLE aliases (
        id         TEXT PRIMARY KEY,
        term       TEXT NOT NULL UNIQUE,
        target     TEXT NOT NULL,
        created_at TEXT NOT NULL
    );

    CREATE TABLE clipboard (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        text       TEXT NOT NULL,
        kind       TEXT NOT NULL,
        source_app TEXT,
        created_at TEXT NOT NULL
    );
    CREATE INDEX clipboard_created ON clipboard(created_at DESC);

    CREATE TABLE usage (
        key       TEXT PRIMARY KEY,
        count     INTEGER NOT NULL,
        last_used TEXT NOT NULL
    );
    CREATE INDEX usage_last_used ON usage(last_used DESC);

    CREATE TABLE applications (
        id          TEXT PRIMARY KEY,
        name        TEXT NOT NULL,
        path        TEXT NOT NULL,
        source      TEXT NOT NULL,
        keywords    TEXT NOT NULL DEFAULT '',
        last_indexed TEXT NOT NULL
    );
    CREATE INDEX applications_name ON applications(name);
    "#,
    // Version 2: the whole-drive file index. `name` and `dir` are
    // denormalised from `path` because the query scans the display name and
    // the per-directory rewrite wants to delete by dir; the directory
    // mtimes decide what the refresh re-lists, kept in nanoseconds since
    // the Unix epoch because a whole-millisecond truncation can bury a
    // write that lands in the same millisecond the pass recorded the
    // directory. `dir` has no unique constraint: one directory's listing
    // is rewritten at a time.
    r#"
    CREATE TABLE file_index (
        path TEXT PRIMARY KEY,
        dir  TEXT NOT NULL,
        name TEXT NOT NULL
    );
    CREATE INDEX file_index_dir ON file_index(dir);
    CREATE INDEX file_index_name ON file_index(name);

    CREATE TABLE index_dirs (
        path     TEXT PRIMARY KEY,
        mtime_ns INTEGER NOT NULL
    );
    "#,
];

/// The statements that create the version-tracking table.
pub const BOOTSTRAP: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);
"#;
