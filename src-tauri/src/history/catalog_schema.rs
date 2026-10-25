//! Version 3 adds query projections without rewriting version 2 metadata.
use rusqlite::{params, Connection, Transaction};
use super::{catalog::{db_error, decode_row}, catalog_model::*};

const SCHEMA: &str = "
CREATE TABLE entry_projection (
    key TEXT PRIMARY KEY NOT NULL,
    listed INTEGER NOT NULL CHECK(listed IN (0,1)),
    archived INTEGER NOT NULL CHECK(archived IN (0,1)),
    pinned INTEGER NOT NULL CHECK(pinned IN (0,1)),
    source TEXT NOT NULL,
    directory TEXT NOT NULL,
    title_search TEXT NOT NULL,
    directory_search TEXT NOT NULL,
    updated_order BLOB NOT NULL CHECK(typeof(updated_order)='blob' AND length(updated_order)=8),
    offline INTEGER NOT NULL CHECK(offline IN (0,1))
);
CREATE INDEX catalog_page_order ON entry_projection(archived, pinned DESC, updated_order DESC, key) WHERE listed=1;
CREATE INDEX catalog_directory_order ON entry_projection(directory, archived, pinned DESC, updated_order DESC, key) WHERE listed=1;
CREATE INDEX catalog_source_order ON entry_projection(source, archived, pinned DESC, updated_order DESC, key) WHERE listed=1;
CREATE INDEX catalog_offline ON entry_projection(key) WHERE offline=1;
CREATE TABLE catalog_projection_dirty (key TEXT PRIMARY KEY NOT NULL);
CREATE TRIGGER catalog_insert_dirty AFTER INSERT ON entries BEGIN
    INSERT OR IGNORE INTO catalog_projection_dirty VALUES(NEW.key);
END;
CREATE TRIGGER catalog_update_dirty AFTER UPDATE OF key, metadata ON entries BEGIN
    DELETE FROM entry_projection WHERE key=OLD.key;
    DELETE FROM catalog_projection_dirty WHERE key=OLD.key;
    INSERT OR IGNORE INTO catalog_projection_dirty VALUES(NEW.key);
END;
CREATE TRIGGER catalog_delete_projection AFTER DELETE ON entries BEGIN
    DELETE FROM entry_projection WHERE key=OLD.key;
    DELETE FROM catalog_projection_dirty WHERE key=OLD.key;
END;
";

pub(super) fn source_name(source: ConversationSource) -> &'static str {
    match source { ConversationSource::LightChat => "light_chat", ConversationSource::Workbench => "workbench", ConversationSource::Legacy => "legacy" }
}

pub(super) fn write_projection(connection: &Connection, key: &str, row: &CatalogEntry) -> Result<(), String> {
    let directory = match &row.identity { CatalogIdentity::Native { directory, .. } => directory.as_str(), CatalogIdentity::Legacy { .. } => "" };
    let listed = row.tombstone.is_none() && row.visible_in_history();
    let offline = listed && matches!(row.identity, CatalogIdentity::Native { .. }) && row.availability != Availability::Available;
    // Big-endian bytes preserve all u64 timestamps, including values above i64::MAX.
    // Rust lowercase preserves the existing Unicode search semantics; SQL lower/LIKE do not.
    connection.prepare_cached("INSERT INTO entry_projection(key, listed, archived, pinned, source, directory, title_search, directory_search, updated_order, offline)
        VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
        ON CONFLICT(key) DO UPDATE SET listed=excluded.listed, archived=excluded.archived, pinned=excluded.pinned,
        source=excluded.source, directory=excluded.directory, title_search=excluded.title_search,
        directory_search=excluded.directory_search, updated_order=excluded.updated_order, offline=excluded.offline")
        .map_err(db_error)?.execute(params![key, listed, row.archived, row.pinned, source_name(row.source), directory,
            row.display_title().to_lowercase(), directory.to_lowercase(), row.updated.to_be_bytes().as_slice(), offline]).map_err(db_error)?;
    connection.prepare_cached("DELETE FROM catalog_projection_dirty WHERE key=?1").map_err(db_error)?.execute([key]).map_err(db_error)?;
    Ok(())
}

pub(super) fn migrate_v3(transaction: &Transaction<'_>) -> Result<(), String> {
    migrate(transaction, || Ok(()))
}

// The callback permits deterministic interruption after partial backfill in tests.
// Production performs no callback work and never commits an incomplete projection.
fn migrate(transaction: &Transaction<'_>, mut after_row: impl FnMut() -> Result<(), String>) -> Result<(), String> {
    transaction.execute_batch(SCHEMA).map_err(db_error)?;
    let mut statement = transaction.prepare("SELECT key, metadata FROM entries ORDER BY key").map_err(db_error)?;
    let mut rows = statement.query([]).map_err(db_error)?;
    while let Some(row) = rows.next().map_err(db_error)? {
        let key: String = row.get(0).map_err(db_error)?;
        let metadata: String = row.get(1).map_err(db_error)?;
        let entry = decode_row(&key, &metadata)?;
        write_projection(transaction, &key, &entry)?;
        after_row()?;
    }
    transaction.pragma_update(None, "user_version", 3).map_err(db_error)?;
    Ok(())
}

#[cfg(test)]
#[path = "catalog_schema_tests.rs"]
mod tests;
