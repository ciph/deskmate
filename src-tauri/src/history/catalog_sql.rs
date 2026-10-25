use rusqlite::{params_from_iter, types::Value, Connection};
use super::{catalog::{CatalogStore, db_error, decode_row}, catalog_query::{CatalogPage, CatalogQuery, CatalogRow}, catalog_schema::source_name};

struct Filter { sql: String, values: Vec<Value> }
impl Filter {
    fn new(query: &CatalogQuery) -> Self {
        let mut filter = Self { sql: "listed=1 AND archived=?".into(), values: vec![Value::Integer(i64::from(query.archived.unwrap_or(false)))] };
        if let Some(pinned) = query.pinned { filter.add("pinned=?", Value::Integer(i64::from(pinned))); }
        if let Some(source) = query.source { filter.add("source=?", Value::Text(source_name(source).into())); }
        if let Some(directory) = &query.directory { filter.add("directory=?", Value::Text(directory.clone())); }
        if let Some(from) = query.from { filter.add("updated_order>=?", Value::Blob(from.to_be_bytes().to_vec())); }
        if let Some(to) = query.to { filter.add("updated_order<=?", Value::Blob(to.to_be_bytes().to_vec())); }
        let search = query.search.as_deref().unwrap_or_default().trim().to_lowercase();
        if !search.is_empty() {
            filter.sql.push_str(" AND (instr(title_search,?)>0 OR instr(directory_search,?)>0)");
            filter.values.extend([Value::Text(search.clone()), Value::Text(search)]);
        }
        filter
    }
    fn add(&mut self, sql: &str, value: Value) { self.sql.push_str(" AND "); self.sql.push_str(sql); self.values.push(value); }
    fn page_sql(&self) -> String {
        // Resolve only the page keys against metadata; filtering/counting never decodes JSON.
        format!("SELECT e.key, e.metadata FROM (SELECT key, pinned, updated_order FROM entry_projection WHERE {} ORDER BY pinned DESC, updated_order DESC, key LIMIT ? OFFSET ?) AS page JOIN entries e ON e.key=page.key ORDER BY page.pinned DESC, page.updated_order DESC, page.key", self.sql)
    }
}

impl CatalogStore {
    pub(super) fn page(&self, query: CatalogQuery, errors: Vec<String>) -> Result<CatalogPage, String> {
        let mut connection = self.connection()?;
        // Count, global offline state, directories and page come from one read snapshot.
        // A DEFERRED read transaction does not reserve against catalog writers.
        let transaction = connection.transaction().map_err(db_error)?;
        let page = read_page(&transaction, query, errors)?;
        transaction.commit().map_err(db_error)?;
        Ok(page)
    }
}

fn read_page(connection: &Connection, query: CatalogQuery, errors: Vec<String>) -> Result<CatalogPage, String> {
    // Unknown/direct SQL metadata writes invalidate the projection via triggers.
    // Never return stale counts or silently hide unprojected/corrupt records.
    let dirty: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM catalog_projection_dirty)", [], |row| row.get(0)).map_err(db_error)?;
    if dirty { return Err("history_catalog_invalid".into()); }
    let filter = Filter::new(&query);
    let count: i64 = connection.query_row(&format!("SELECT count(*) FROM entry_projection WHERE {}", filter.sql), params_from_iter(&filter.values), |row| row.get(0)).map_err(db_error)?;
    let total = usize::try_from(count).map_err(|_| "history_catalog_invalid")?;
    let offline = connection.query_row("SELECT EXISTS(SELECT 1 FROM entry_projection WHERE offline=1)", [], |row| row.get(0)).map_err(db_error)?;
    let directories = {
        let mut statement = connection.prepare("SELECT directory FROM directories ORDER BY directory").map_err(db_error)?;
        let rows = statement.query_map([], |row| row.get(0)).map_err(db_error)?;
        rows.collect::<Result<Vec<String>, _>>().map_err(db_error)?
    };
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let mut items = Vec::new();
    // Avoid wrapping usize into a negative SQLite OFFSET (which would return page one).
    if offset < total {
        let sql = filter.page_sql();
        let mut values = filter.values;
        values.extend([Value::Integer(limit as i64), Value::Integer(i64::try_from(offset).map_err(|_| "history_catalog_invalid")?)]);
        let mut statement = connection.prepare(&sql).map_err(db_error)?;
        let mut rows = statement.query(params_from_iter(&values)).map_err(db_error)?;
        while let Some(row) = rows.next().map_err(db_error)? {
            items.push(CatalogRow::from(decode_row(&row.get::<_, String>(0).map_err(db_error)?, &row.get::<_, String>(1).map_err(db_error)?)?));
        }
    }
    Ok(CatalogPage { has_more: offset.saturating_add(items.len()) < total, items, total, directories, offline, errors })
}

#[cfg(test)]
#[path = "catalog_sql_tests.rs"]
pub(super) mod tests;
