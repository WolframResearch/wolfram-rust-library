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
use wolfram_expr::{expr, Expr};
use wolfram_serialize::ToWXF;

struct DuckDbError {
    code: Option<i32>,
    message: String,
}

impl From<duckdb::Error> for DuckDbError {
    fn from(e: duckdb::Error) -> Self {
        match e {
            duckdb::Error::DuckDBFailure(ffi_err, msg) => Self {
                code: Some(ffi_err.extended_code as i32),
                message: msg.unwrap_or_else(|| {
                    format!("DuckDB error code {}", ffi_err.extended_code)
                }),
            },
            other => Self {
                code: None,
                message: other.to_string(),
            },
        }
    }
}

/// Every DuckDB operation resolves to one of these. Per-variant `#[wolfram(enum_head)]`
/// makes the success branches serialize under `System`Success` and the error
/// branches under the container default `System`Failure`; `CamelCase` keys give
/// the failure associations Wolfram-style keys.
#[derive(Debug, ToWXF)]
#[wolfram(enum_head = "System`Failure", key_processor = "CamelCase")]
enum DuckDbResult {
    /// A connection was opened or closed → the handle id directly (transparent).
    #[wolfram(enum_head = false)]
    ConnectionResult(String),
    /// A query ran → the decoded table `Expr` directly (transparent).
    #[wolfram(enum_head = false)]
    ExprResult(Expr),
    /// Opening / attaching a connection failed → `Failure["ConnectionError", <|"Code" -> …, "Message" -> …|>]`.
    ConnectionError { code: Option<i32>, message: String },
    /// Preparing a statement failed → `Failure["PrepareError", <|"Code" -> …, "Message" -> …|>]`.
    PrepareError { code: Option<i32>, message: String },
    /// Executing a query failed → `Failure["ExecuteError", <|"Code" -> …, "Message" -> …|>]`.
    ExecuteError { code: Option<i32>, message: String },
    /// Serializing Arrow batches to IPC failed → `Failure["SerializationError", <|"Message" -> …|>]`.
    /// Arrow IPC errors carry no numeric code, so there's no `Code` key here.
    SerializationError { message: String },
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
/// string directly (transparent `ConnectionResult`) — or `Failure["ConnectionError", …]`.
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
                    let DuckDbError { code, message } = e.into();
                    return DuckDbResult::ConnectionError { code, message };
                },
            }
        },
        Target::Attach { url, db_type } => {
            let conn = match Connection::open_in_memory() {
                Ok(conn) => conn,
                Err(e) => {
                    let DuckDbError { code, message } = e.into();
                    return DuckDbResult::ConnectionError { code, message };
                },
            };
            if let Err(e) =
                conn.execute(&format!("ATTACH '{url}' AS remote (TYPE {db_type})"), [])
            {
                let DuckDbError { code, message } = e.into();
                return DuckDbResult::ConnectionError { code, message };
            }
            if let Err(e) = conn.execute("USE remote", []) {
                let DuckDbError { code, message } = e.into();
                return DuckDbResult::ConnectionError { code, message };
            }
            conn
        },
    };

    let id = Uuid::new_v4().to_string();
    registry().lock().unwrap().insert(id.clone(), conn);
    DuckDbResult::ConnectionResult(id)
}

/// Run `sql` on connection `id`. On success returns the decoded table
/// `ImportByteArray[ByteArray[…], "ArrowIPC"]` **directly** (the `Result` variant
/// is transparent — `enum_head = false`). On failure returns
/// `Failure["UnknownConnection", …]` (no such handle), `Failure["PrepareError", …]`,
/// `Failure["ExecuteError", …]`, or `Failure["SerializationError", …]`.
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
            let DuckDbError { code, message } = e.into();
            return DuckDbResult::PrepareError { code, message };
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
            let DuckDbError { code, message } = e.into();
            return DuckDbResult::ExecuteError { code, message };
        },
    };

    let schema = arrow.get_schema();
    let batches: Vec<RecordBatch> = arrow.collect();

    // Encode the Arrow batches as an IPC stream.
    let mut buf = Vec::new();
    {
        let mut writer = match arrow_ipc::writer::StreamWriter::try_new(&mut buf, &schema)
        {
            Ok(writer) => writer,
            Err(e) => {
                return DuckDbResult::SerializationError {
                    message: e.to_string(),
                }
            },
        };
        for batch in &batches {
            if let Err(e) = writer.write(batch) {
                return DuckDbResult::SerializationError {
                    message: e.to_string(),
                };
            }
        }
        if let Err(e) = writer.finish() {
            return DuckDbResult::SerializationError {
                message: e.to_string(),
            };
        }
    }

    let byte_array = Expr::from(buf);
    // Tabular`Arrow`ToTabular[Tabular`Arrow`ReadArrowIPCByteArray[ba]] — the
    // internal path Import[…, "ArrowIPC", "Tabular"] uses, skipping the importer.
    // `::` in expr! builds a context-qualified symbol; `byte_array` (no `::`) is
    // the local variable.
    DuckDbResult::ExprResult(expr!(
        Tabular::Arrow::ToTabular[Tabular::Arrow::ReadArrowIPCByteArray[byte_array]]
    ))
}

/// Close connection `id`, returning `id` directly — or
/// `Failure["UnknownConnection", …]` if there is no open handle with that id.
#[export(wxf)]
fn db_disconnect(id: String) -> DuckDbResult {
    match registry().lock().unwrap().remove(&id) {
        Some(_) => DuckDbResult::ConnectionResult(id),
        None => DuckDbResult::UnknownConnection { id },
    }
}
