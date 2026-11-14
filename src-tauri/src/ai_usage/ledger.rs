//! Message rows are authoritative; the current local-day window is a rebuildable read model.
use super::{LocalSummary, LocalUsageModel, StoredUsage, LEDGER_FILE};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::de::{MapAccess, Visitor};
use serde::Deserializer;
use std::{collections::HashMap, fs, io::BufReader, path::Path};

pub(super) const DB_FILE: &str = "ai-usage-local.sqlite3";
const SCHEMA: &str = include_str!("ledger.sql");
const INSERT: &str = "INSERT INTO usage_messages(message_id,provider_id,model_id,created_at_ms,tokens) VALUES(?1,?2,?3,?4,?5)";

pub(super) struct Ledger { connection: Connection }
fn storage_error(_: rusqlite::Error) -> String { "usage_storage_unavailable".into() }
fn invalid() -> String { "usage_storage_invalid".into() }
fn parse<T: std::str::FromStr>(value: String) -> Result<T, String> { value.parse().map_err(|_| invalid()) }

impl Ledger {
    pub(super) fn open(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|_| "usage_storage_unavailable")?;
        let mut connection = Connection::open(root.join(DB_FILE)).map_err(storage_error)?;
        connection.busy_timeout(std::time::Duration::from_secs(5)).map_err(storage_error)?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(storage_error)?;
        if version > 1 { return Err(invalid()); }
        // FULL retains the old sync_all durability contract; never trade saved usage for speed.
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;").map_err(storage_error)?;
        if version == 0 {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
            let current: i64 = tx.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(storage_error)?;
            if current > 1 { return Err(invalid()); }
            if current == 0 {
                tx.execute_batch(SCHEMA).map_err(storage_error)?;
                import_json(&tx, &root.join(LEDGER_FILE))?;
                tx.pragma_update(None, "user_version", 1).map_err(storage_error)?;
            }
            tx.commit().map_err(storage_error)?;
        }
        Ok(Self { connection })
    }

    pub(super) fn save(&mut self, entries: impl IntoIterator<Item=(String, StoredUsage)>) -> Result<(), String> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
        for (id, next) in entries {
            let old: Option<(String, String, i64, String)> = tx.query_row(
                "SELECT provider_id,model_id,created_at_ms,tokens FROM usage_messages WHERE message_id=?1", [&id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).optional().map_err(storage_error)?;
            let (delta, requests) = match old {
                Some((provider, model, created, tokens)) => {
                    if provider != next.provider_id || model != next.model_id || created != next.created_at_ms {
                        return Err("usage_message_identity_mismatch".into());
                    }
                    let previous: u64 = parse(tokens)?;
                    if next.tokens <= previous { continue; }
                    tx.execute("UPDATE usage_messages SET tokens=?1 WHERE message_id=?2", params![next.tokens.to_string(),id]).map_err(storage_error)?;
                    (next.tokens - previous, 0)
                }
                None => {
                    tx.execute(INSERT, params![id,next.provider_id,next.model_id,next.created_at_ms,next.tokens.to_string()]).map_err(storage_error)?;
                    (next.tokens, 1)
                }
            };
            let cached = tx.query_row(
                "SELECT 1 FROM usage_summary_windows WHERE provider_id=?1 AND start_ms<=?2 AND end_ms>?2",
                params![next.provider_id,next.created_at_ms], |_| Ok(())).optional().map_err(storage_error)?.is_some();
            if cached { add_summary(&tx, &next.provider_id, &next.model_id, u128::from(delta), requests)?; }
        }
        tx.commit().map_err(storage_error)
    }

    pub(super) fn summary(&mut self, provider: &str, start: i64, end: i64) -> Result<LocalSummary, String> {
        let tx = self.connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(storage_error)?;
        let window = tx.query_row("SELECT start_ms,end_ms FROM usage_summary_windows WHERE provider_id=?1", [provider],
            |r| Ok((r.get::<_,i64>(0)?,r.get::<_,i64>(1)?))).optional().map_err(storage_error)?;
        if window != Some((start,end)) {
            // Changing local day/timezone invalidates only this provider's day, not all history.
            tx.execute("DELETE FROM usage_summary_windows WHERE provider_id=?1", [provider]).map_err(storage_error)?;
            tx.execute("INSERT INTO usage_summary_windows VALUES(?1,?2,?3)", params![provider,start,end]).map_err(storage_error)?;
            let mut models: HashMap<String,(u128,u64)> = HashMap::new();
            let mut statement = tx.prepare("SELECT model_id,tokens FROM usage_messages WHERE provider_id=?1 AND created_at_ms>=?2 AND created_at_ms<?3").map_err(storage_error)?;
            let rows = statement.query_map(params![provider,start,end], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(storage_error)?;
            for row in rows {
                let (model,tokens) = row.map_err(storage_error)?;
                let total = models.entry(model).or_default();
                total.0 = total.0.checked_add(u128::from(parse::<u64>(tokens)?)).ok_or_else(invalid)?;
                total.1 = total.1.checked_add(1).ok_or_else(invalid)?;
            }
            for (model,(tokens,requests)) in models { add_summary(&tx,provider,&model,tokens,requests)?; }
        }
        let mut summary = LocalSummary::default();
        {
            let mut statement = tx.prepare("SELECT model_id,tokens,requests FROM usage_summary_models WHERE provider_id=?1").map_err(storage_error)?;
            let rows = statement.query_map([provider], |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,i64>(2)?))).map_err(storage_error)?;
            for row in rows {
                let (name,tokens,requests) = row.map_err(storage_error)?;
                let requests = u64::try_from(requests).map_err(|_| invalid())?;
                let tokens = parse::<u128>(tokens)?.min(u128::from(u64::MAX)) as u64;
                summary.tokens = summary.tokens.saturating_add(tokens);
                summary.requests = summary.requests.saturating_add(requests);
                summary.top_models.push(LocalUsageModel { name,tokens,requests });
            }
        }
        summary.top_models.sort_by(|a,b| b.tokens.cmp(&a.tokens).then_with(|| a.name.cmp(&b.name)));
        summary.top_models.truncate(3);
        tx.commit().map_err(storage_error)?;
        Ok(summary)
    }
}

fn add_summary(db: &Connection, provider: &str, model: &str, delta: u128, requests: u64) -> Result<(), String> {
    let previous = db.query_row("SELECT tokens,requests FROM usage_summary_models WHERE provider_id=?1 AND model_id=?2",
        params![provider,model], |r| Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).optional().map_err(storage_error)?;
    let (tokens,count) = match previous { Some((tokens,count)) => (parse::<u128>(tokens)?,u64::try_from(count).map_err(|_| invalid())?), None => (0,0) };
    let tokens = tokens.checked_add(delta).ok_or_else(invalid)?;
    let count = i64::try_from(count.checked_add(requests).ok_or_else(invalid)?).map_err(|_| invalid())?;
    db.execute("INSERT INTO usage_summary_models VALUES(?1,?2,?3,?4) ON CONFLICT(provider_id,model_id) DO UPDATE SET tokens=excluded.tokens,requests=excluded.requests",
        params![provider,model,tokens.to_string(),count]).map_err(storage_error)?;
    Ok(())
}

fn import_json(db: &Connection, path: &Path) -> Result<(), String> {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind()==std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("usage_storage_unavailable".into()),
    };
    // Stream one message at a time. Legacy duplicate JSON keys retain serde HashMap's last-value behavior.
    struct Import<'a,'b> { statement: &'a mut rusqlite::Statement<'b>, sql_failed: &'a mut bool }
    impl<'de> Visitor<'de> for Import<'_, '_> {
        type Value = ();
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str("usage message map") }
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(),M::Error> {
            while let Some((id,entry)) = map.next_entry::<String,StoredUsage>()? {
                if self.statement.execute(params![id,entry.provider_id,entry.model_id,entry.created_at_ms,entry.tokens.to_string()]).is_err() {
                    *self.sql_failed = true;
                    return Err(serde::de::Error::custom("usage import failed"));
                }
            }
            Ok(())
        }
    }
    let mut statement = db.prepare(&format!("{INSERT} ON CONFLICT(message_id) DO UPDATE SET provider_id=excluded.provider_id,model_id=excluded.model_id,created_at_ms=excluded.created_at_ms,tokens=excluded.tokens")).map_err(storage_error)?;
    let mut sql_failed = false;
    let mut reader = serde_json::Deserializer::from_reader(BufReader::new(file));
    let result = (&mut reader).deserialize_map(Import { statement: &mut statement,sql_failed: &mut sql_failed }).and_then(|()| reader.end());
    result.map_err(|error| if sql_failed || error.is_io() { "usage_storage_unavailable".into() } else { invalid() })
}

#[cfg(test)]
#[path = "ledger_tests.rs"]
mod tests;
