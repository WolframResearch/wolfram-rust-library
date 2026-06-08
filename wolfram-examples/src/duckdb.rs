//! DuckDB example — open an in-memory (or file) DuckDB connection, run SQL,
//! and return results as Arrow IPC bytes. DuckDB is statically compiled in via
//! `bundled`; `vtab-arrow` lets us call `query_arrow()` so the engine converts
//! results to Arrow RecordBatches natively — no manual type mapping needed.
//!
//! Exposed WXF functions (in `examples/duckdb.rs`):
//! ```text
//! db_connect(url)            -> Result<handle, String>
//! db_query(id, sql, params)  -> Result<Expr, String>   (ImportByteArray[…, "ArrowIPC"])
//! db_disconnect(id)          -> Result<handle, String>
//! ```
//! URL: `duckdb://` or `duckdb://:memory:` for in-memory; `duckdb:///path/to/file.db`.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use duckdb::arrow::record_batch::RecordBatch;
use duckdb::{Connection, types::ToSql};
use uuid::Uuid;

pub type DbError = String;

fn dberr(context: &str, e: impl std::fmt::Display) -> DbError {
    format!("{context}: {e}")
}

// Process-global connection registry keyed by uuid handle.
fn registry() -> &'static Mutex<HashMap<String, Connection>> {
    static REG: OnceLock<Mutex<HashMap<String, Connection>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

enum Target {
    /// Open DuckDB directly at this path (`:memory:` for in-memory).
    DuckDb(String),
    /// Open in-memory DuckDB, then ATTACH a foreign database. DuckDB
    /// autoloads the required extension (postgres, sqlite, mysql) on first use.
    /// After ATTACH + USE, queries run against the foreign tables transparently.
    Attach { url: String, db_type: String },
}

fn parse_url(url: &str) -> Target {
    if let Some(rest) = url.strip_prefix("duckdb://") {
        let path = if rest.is_empty() || rest == ":memory:" { ":memory:".into() } else { rest.to_string() };
        return Target::DuckDb(path);
    }
    let scheme = url.split("://").next().unwrap_or("unknown");
    let db_type = match scheme {
        "postgres" | "postgresql" => "postgres",
        "sqlite"                  => "sqlite",
        "mysql"                   => "mysql",
        other                     => other,
    };
    // SQLite ATTACH takes a file path, not a full URL.
    let attach_str = if scheme == "sqlite" {
        url.strip_prefix("sqlite://").unwrap_or(url).to_string()
    } else {
        url.to_string()
    };
    Target::Attach { url: attach_str, db_type: db_type.to_string() }
}

/// Open a connection and return a uuid handle.
///
/// - `duckdb://` or `duckdb://:memory:` — in-memory DuckDB.
/// - `duckdb:///path/to/file.db` — file-backed DuckDB.
/// - `postgres://user:pass@host/db` — DuckDB in-memory + ATTACH postgres
///   (DuckDB autoloads the postgres extension; queries use native DuckDB SQL).
/// - `sqlite:///path/to/file.db` — DuckDB in-memory + ATTACH sqlite file.
pub fn connect(url: &str) -> Result<String, DbError> {
    let conn = match parse_url(url) {
        Target::DuckDb(path) => {
            if path == ":memory:" {
                Connection::open_in_memory()
            } else {
                Connection::open(&path)
            }
            .map_err(|e| dberr("open connection", e))?
        }
        Target::Attach { url: attach_url, db_type } => {
            let conn = Connection::open_in_memory()
                .map_err(|e| dberr("open connection", e))?;
            conn.execute(
                &format!("ATTACH '{attach_url}' AS remote (TYPE {db_type})"),
                [],
            )
            .map_err(|e| dberr("attach database", e))?;
            conn.execute("USE remote", [])
                .map_err(|e| dberr("use database", e))?;
            conn
        }
    };

    let id = Uuid::new_v4().to_string();
    registry().lock().unwrap().insert(id.clone(), conn);
    Ok(id)
}

/// Run `sql` on connection `id` and return Arrow IPC stream bytes.
///
/// DuckDB converts results to Arrow RecordBatches natively — integers, floats,
/// strings, booleans, blobs, lists, structs all arrive with correct Arrow types.
///
/// `params` are bound positionally (sorted by key name); use `?` placeholders
/// in SQL for each param in sorted key order.
pub fn query(id: &str, sql: &str, params: HashMap<String, String>) -> Result<Vec<u8>, DbError> {
    let reg = registry().lock().unwrap();
    let conn = reg
        .get(id)
        .ok_or_else(|| format!("unknown connection: {id}"))?;

    let mut stmt = conn.prepare(sql).map_err(|e| dberr("prepare", e))?;

    let arrow = if params.is_empty() {
        stmt.query_arrow([]).map_err(|e| dberr("execute", e))?
    } else {
        let mut keys: Vec<&String> = params.keys().collect();
        keys.sort();
        let vals: Vec<&str> = keys.iter().map(|k| params[*k].as_str()).collect();
        let to_sql: Vec<&dyn ToSql> =
            vals.iter().map(|v| v as &dyn ToSql).collect();
        stmt.query_arrow(to_sql.as_slice()).map_err(|e| dberr("execute", e))?
    };

    let schema = arrow.get_schema();
    let batches: Vec<RecordBatch> = arrow.collect();
    encode_ipc(&schema, &batches)
}

/// Close the connection `id`; returns the id on success.
pub fn disconnect(id: &str) -> Result<String, DbError> {
    registry()
        .lock()
        .unwrap()
        .remove(id)
        .map(|_| id.to_string())
        .ok_or_else(|| format!("unknown connection: {id}"))
}

fn encode_ipc(
    schema: &duckdb::arrow::datatypes::SchemaRef,
    batches: &[RecordBatch],
) -> Result<Vec<u8>, DbError> {
    let mut buf = Vec::new();
    {
        let mut writer = arrow_ipc::writer::StreamWriter::try_new(&mut buf, schema)
            .map_err(|e| dberr("serialize", e))?;
        for batch in batches {
            writer.write(batch).map_err(|e| dberr("serialize", e))?;
        }
        writer.finish().map_err(|e| dberr("serialize", e))?;
    }
    Ok(buf)
}
