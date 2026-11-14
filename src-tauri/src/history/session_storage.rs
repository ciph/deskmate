//! Canonical per-session history, lazily imported from the untouched JSON backup.
//! Every published write is one FULL-synchronous SQLite transaction.
use std::{path::{Path, PathBuf}, time::Duration};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use super::{HistorySession, storage::{PreparedHistoryFile, prepare_path}};

fn error(_: impl std::fmt::Display) -> String { "history_storage_unavailable".into() }
pub(super) fn path(path: &Path) -> PathBuf { path.with_extension("sqlite") }
fn connection(path: &Path, wait: bool) -> Result<Connection, String> {
    let connection = Connection::open(self::path(path)).map_err(error)?;
    connection.busy_timeout(if wait { Duration::from_secs(5) } else { Duration::ZERO }).map_err(error)?;
    connection.pragma_update(None, "synchronous", "FULL").map_err(error)?;
    Ok(connection)
}
pub(super) fn ready(path: &Path) -> Result<bool, String> {
    if !self::path(path).exists() { return Ok(false); }
    let version: u32 = connection(path, true)?.pragma_query_value(None,"user_version", |r|r.get(0)).map_err(error)?;
    match version { 0 => Ok(false), 1 => Ok(true), _ => Err("history_version_unsupported".into()) }
}
pub(super) fn ensure(path: &Path) -> Result<(), String> {
    if ready(path)? { return Ok(()); }
    std::fs::create_dir_all(path.parent().ok_or("history_storage_unavailable")?).map_err(error)?;
    let mut connection = connection(path, true)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(error)?;
    let version: u32 = tx.pragma_query_value(None,"user_version", |r|r.get(0)).map_err(error)?;
    if version == 1 { return Ok(()); }
    if version != 0 { return Err("history_version_unsupported".into()); }
    tx.execute_batch("CREATE TABLE sessions(id TEXT PRIMARY KEY NOT NULL, updated BLOB NOT NULL, metadata TEXT NOT NULL, position INTEGER NOT NULL); CREATE INDEX sessions_order ON sessions(updated DESC,position);").map_err(error)?;
    import_json(&tx, path)?;
    tx.pragma_update(None,"user_version",1).map_err(error)?;
    tx.commit().map_err(error)
}

fn import_json(tx: &rusqlite::Transaction<'_>, path: &Path) -> Result<(), String> {
    use serde::de::{Deserializer, Error, SeqAccess, Visitor};
    let file=match std::fs::File::open(path) {
        Ok(file)=>file,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(()),
        Err(e)=>return Err(error(e)),
    };
    struct Import<'a,'b>(&'a rusqlite::Transaction<'b>, &'a mut Option<String>);
    impl<'de> Visitor<'de> for Import<'_,'_> {
        type Value=();
        fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result { f.write_str("history session array") }
        fn visit_seq<A:SeqAccess<'de>>(self,mut sequence:A)->Result<(),A::Error> {
            let mut position=0_i64;
            while let Some(session)=sequence.next_element::<HistorySession>()? {
                let result=(|| {
                    let json=serde_json::to_string(&session).map_err(error)?;
                    self.0.execute("INSERT INTO sessions VALUES(?1,?2,?3,?4)",rusqlite::params![session.id,session.updated.to_be_bytes().as_slice(),json,position]).map_err(error)?;
                    Ok::<_,String>(())
                })();
                if let Err(error)=result { *self.1=Some(error); return Err(A::Error::custom("history import failed")); }
                position=position.checked_add(1).ok_or_else(||A::Error::custom("history count overflow"))?;
            }
            Ok(())
        }
    }
    let mut failure=None;
    let mut decoder=serde_json::Deserializer::from_reader(std::io::BufReader::new(file));
    decoder.deserialize_seq(Import(tx,&mut failure)).and_then(|_|decoder.end())
        .map_err(|e|failure.unwrap_or_else(||if e.is_io() {error(e)} else {"history_invalid".into()}))
}
pub(super) fn load(path: &Path) -> Result<Vec<HistorySession>, String> {
    let connection=connection(path,true)?;
    let mut statement=connection.prepare("SELECT id,metadata FROM sessions ORDER BY updated DESC,position").map_err(error)?;
    let rows=statement.query_map([],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).map_err(error)?;
    rows.map(|row| { let (id,json)=row.map_err(error)?; let session:HistorySession=serde_json::from_str(&json).map_err(|_|"history_invalid")?;
        if session.id!=id { return Err("history_invalid".into()); } Ok(session) }).collect()
}
pub(super) fn replace_all(path: &Path, sessions: &[HistorySession]) -> Result<(), String> {
    let mut connection=connection(path,true)?;
    let tx=connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(error)?;
    if tx.pragma_query_value(None,"user_version",|row|row.get::<_,u32>(0)).map_err(error)? != 1 { return Err("history_version_unsupported".into()); }
    tx.execute_batch("CREATE TEMP TABLE wanted(id TEXT PRIMARY KEY NOT NULL);").map_err(error)?;
    for (position,session) in sessions.iter().enumerate() {
        tx.execute("INSERT INTO wanted VALUES(?1)",[&session.id]).map_err(error)?;
        let json=serde_json::to_string(session).map_err(error)?;
        tx.execute("INSERT INTO sessions VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET updated=excluded.updated,metadata=excluded.metadata,position=excluded.position WHERE sessions.metadata<>excluded.metadata OR sessions.position<>excluded.position",rusqlite::params![session.id,session.updated.to_be_bytes().as_slice(),json,position as i64]).map_err(error)?;
    }
    tx.execute("DELETE FROM sessions WHERE id NOT IN (SELECT id FROM wanted)",[]).map_err(error)?;
    tx.commit().map_err(error)
}

pub(super) struct PreparedSessionFile {
    staged: PreparedHistoryFile,
    path: PathBuf,
    baseline: Option<String>,
    id: String,
    updated: u64,
}
pub(super) fn prepare(path: &Path, previous: &HistorySession, session: &HistorySession) -> Result<PreparedSessionFile, String> {
    ensure(path)?;
    let baseline: Option<String>=connection(path,true)?.query_row("SELECT metadata FROM sessions WHERE id=?1",[&session.id],|r|r.get(0)).optional().map_err(error)?;
    if baseline.is_none() { return Err("history_snapshot_superseded".into()); }
    if let Some(json) = &baseline {
        let stored: HistorySession = serde_json::from_str(json).map_err(|_| "history_invalid")?;
        if &stored != previous { return Err("history_snapshot_superseded".into()); }
    }
    Ok(PreparedSessionFile { staged: prepare_path(path,std::slice::from_ref(session))?, path:path.to_owned(),baseline,id:session.id.clone(),updated:session.updated })
}
impl PreparedSessionFile {
    pub(super) fn commit(self) -> Result<(), String> {
        let json=std::fs::read(&self.staged.pending).map_err(error)?;
        let list:Vec<HistorySession>=serde_json::from_slice(&json).map_err(error)?;
        if list.len()!=1 || list[0].id!=self.id { return Err("history_invalid".into()); }
        let json=serde_json::to_string(&list[0]).map_err(error)?;
        let mut connection=connection(&self.path,false)?;
        let tx=connection.transaction_with_behavior(TransactionBehavior::Immediate).map_err(error)?;
        if tx.pragma_query_value(None,"user_version",|row|row.get::<_,u32>(0)).map_err(error)? != 1 { return Err("history_version_unsupported".into()); }
        let current:Option<String>=tx.query_row("SELECT metadata FROM sessions WHERE id=?1",[&self.id],|r|r.get(0)).optional().map_err(error)?;
        if current!=self.baseline { return Err("history_snapshot_superseded".into()); }
        let changed = tx.execute("UPDATE sessions SET updated=?2,metadata=?3 WHERE id=?1",rusqlite::params![self.id,self.updated.to_be_bytes().as_slice(),json]).map_err(error)?;
        if changed != 1 { return Err("history_snapshot_superseded".into()); }
        tx.commit().map_err(error)
        // Staged file is discarded on drop after successful transaction as well.
    }
}
