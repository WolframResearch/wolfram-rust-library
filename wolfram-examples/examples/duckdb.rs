//! DuckDB example — everything in one file: open a connection, run SQL, and
//! return results as a decoded Arrow table. DuckDB is statically compiled in via
//! `bundled`; `vtab-arrow` lets us call `query_arrow()` so the engine emits Arrow
//! RecordBatches natively — no manual type mapping needed.
//!
//! Each exported function returns one [`DuckDbResult`]. Its **per-variant
//! `enum_head`** puts the success branches under `System`Success` and the error
//! branches under the container default `System`Failure`, so the kernel always
//! receives `Success[tag, payload]` or `Failure["…Error", <|…|>]` — and each
//! function builds those variants directly, with no intermediate `Result`.
//!
//! URL: `duckdb://` or `duckdb://:memory:` for in-memory; `duckdb:///path/to/file.db`;
//! `postgres://…` / `sqlite://…` / `mysql://…` ATTACH a foreign database.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use duckdb::arrow::record_batch::RecordBatch;
use duckdb::{types::ToSql, Connection};
use uuid::Uuid;
use wolfram_export::export;
use wolfram_expr::{expr, Expr, ToWXF};

/// Every DuckDB operation resolves to one of these. Per-variant `#[wolfram(enum_head)]`
/// makes the success branches serialize under `System`Success` and the error
/// branches under the container default `System`Failure`; `CamelCase` keys give
/// the failure associations Wolfram-style keys.
#[derive(Debug, ToWXF)]
#[wolfram(enum_head = "System`Failure", key_processor = "CamelCase")]
enum DuckDbResult {
    /// A connection was opened → the handle id **itself** (transparent,
    /// `enum_head = false`): `db_connect` returns the uuid string directly.
    #[wolfram(enum_head = false)]
    ConnectionOpened(String),
    /// A query ran → the decoded table `Expr` **itself**. `enum_head = false` is
    /// transparent: it drops the head *and* the tag, so the query returns
    /// `ImportByteArray[…, "ArrowIPC"]` directly (no `Success[…]` wrapper).
    #[wolfram(enum_head = false)]
    Result(Expr),
    /// A connection was closed → `Success["ConnectionClosed", id]`.
    #[wolfram(enum_head = "System`Success")]
    ConnectionClosed(String),
    /// Opening / attaching a connection failed → `Failure["ConnectionError", <|"Message" -> …|>]`.
    ConnectionError { message: String },
    /// Preparing / executing / serializing a query failed → `Failure["QueryError", <|"Message" -> …|>]`.
    QueryError { message: String },
    /// No open connection has this handle → `Failure["UnknownConnection", <|"Id" -> …|>]`.
    UnknownConnection { id: String },
}

// Process-global connection registry keyed by uuid handle.
fn registry() -> &'static Mutex<HashMap<String, Connection>> {
    static REG: OnceLock<Mutex<HashMap<String, Connection>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

enum Target {
    /// Open DuckDB directly at this path (`:memory:` for in-memory).
    DuckDb(String),
    /// Open in-memory DuckDB, then ATTACH a foreign database. DuckDB autoloads
    /// the required extension (postgres, sqlite, mysql) on first use.
    Attach { url: String, db_type: String },
}

fn parse_url(url: &str) -> Target {
    if let Some(rest) = url.strip_prefix("duckdb://") {
        let path = if rest.is_empty() || rest == ":memory:" {
            ":memory:".into()
        } else {
            rest.to_string()
        };
        return Target::DuckDb(path);
    }
    let scheme = url.split("://").next().unwrap_or("unknown");
    let db_type = match scheme {
        "postgres" | "postgresql" => "postgres",
        "sqlite" => "sqlite",
        "mysql" => "mysql",
        other => other,
    };
    // SQLite ATTACH takes a file path, not a full URL.
    let attach_str = if scheme == "sqlite" {
        url.strip_prefix("sqlite://").unwrap_or(url).to_string()
    } else {
        url.to_string()
    };
    Target::Attach {
        url: attach_str,
        db_type: db_type.to_string(),
    }
}

/// Open a connection, register it under a fresh uuid handle, and return the id
/// string directly (transparent `ConnectionOpened`) — or `Failure["ConnectionError", …]`.
#[export(wxf)]
fn db_connect(url: String) -> DuckDbResult {
    let conn = match parse_url(&url) {
        Target::DuckDb(path) => {
            let opened = if path == ":memory:" {
                Connection::open_in_memory()
            } else {
                Connection::open(&path)
            };
            match opened {
                Ok(conn) => conn,
                Err(e) => {
                    return DuckDbResult::ConnectionError {
                        message: format!("open connection: {e}"),
                    }
                },
            }
        },
        Target::Attach { url, db_type } => {
            let conn = match Connection::open_in_memory() {
                Ok(conn) => conn,
                Err(e) => {
                    return DuckDbResult::ConnectionError {
                        message: format!("open connection: {e}"),
                    }
                },
            };
            if let Err(e) =
                conn.execute(&format!("ATTACH '{url}' AS remote (TYPE {db_type})"), [])
            {
                return DuckDbResult::ConnectionError {
                    message: format!("attach database: {e}"),
                };
            }
            if let Err(e) = conn.execute("USE remote", []) {
                return DuckDbResult::ConnectionError {
                    message: format!("use database: {e}"),
                };
            }
            conn
        },
    };

    let id = Uuid::new_v4().to_string();
    registry().lock().unwrap().insert(id.clone(), conn);
    DuckDbResult::ConnectionOpened(id)
}

/// Run `sql` on connection `id`. On success returns the decoded table
/// `ImportByteArray[ByteArray[…], "ArrowIPC"]` **directly** (the `Result` variant
/// is transparent — `enum_head = false`). On failure returns
/// `Failure["UnknownConnection", …]` (no such handle) or `Failure["QueryError", …]`.
///
/// `params` are bound positionally (sorted by key name); use `?` placeholders
/// in SQL for each param in sorted key order.
#[export(wxf)]
fn db_query(id: String, sql: String, params: HashMap<String, String>) -> DuckDbResult {
    let reg = registry().lock().unwrap();
    let conn = match reg.get(&id) {
        Some(conn) => conn,
        None => return DuckDbResult::UnknownConnection { id },
    };

    let mut stmt = match conn.prepare(&sql) {
        Ok(stmt) => stmt,
        Err(e) => {
            return DuckDbResult::QueryError {
                message: format!("prepare: {e}"),
            }
        },
    };

    // Bind params positionally, sorted by key (empty slice = no params).
    let mut keys: Vec<&String> = params.keys().collect();
    keys.sort();
    let vals: Vec<&str> = keys.iter().map(|k| params[*k].as_str()).collect();
    let to_sql: Vec<&dyn ToSql> = vals.iter().map(|v| v as &dyn ToSql).collect();

    let arrow = match stmt.query_arrow(to_sql.as_slice()) {
        Ok(arrow) => arrow,
        Err(e) => {
            return DuckDbResult::QueryError {
                message: format!("execute: {e}"),
            }
        },
    };

    let schema = arrow.get_schema();
    let batches: Vec<RecordBatch> = arrow.collect();

    // Encode the Arrow batches as an IPC stream.
    let mut buf = Vec::new();
    {
        let mut writer = match arrow_ipc::writer::StreamWriter::try_new(&mut buf, &schema) {
            Ok(writer) => writer,
            Err(e) => {
                return DuckDbResult::QueryError {
                    message: format!("serialize: {e}"),
                }
            },
        };
        for batch in &batches {
            if let Err(e) = writer.write(batch) {
                return DuckDbResult::QueryError {
                    message: format!("serialize: {e}"),
                };
            }
        }
        if let Err(e) = writer.finish() {
            return DuckDbResult::QueryError {
                message: format!("serialize: {e}"),
            };
        }
    }

    let byte_array = Expr::from(buf);
    DuckDbResult::Result(expr!(ImportByteArray[byte_array, "ArrowIPC"]))
}

/// Close connection `id`, returning `Success["ConnectionClosed", id]` — or
/// `Failure["UnknownConnection", …]` if there is no open handle with that id.
#[export(wxf)]
fn db_disconnect(id: String) -> DuckDbResult {
    match registry().lock().unwrap().remove(&id) {
        Some(_) => DuckDbResult::ConnectionClosed(id),
        None => DuckDbResult::UnknownConnection { id },
    }
}
