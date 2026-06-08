use std::collections::HashMap;

use wolfram_export::export;
use wolfram_expr::{expr, Expr};

#[export(wxf)]
fn db_connect(url: String) -> Result<String, String> {
    wolfram_examples::duckdb::connect(&url)
}

/// Returns `ImportByteArray[ByteArray[…], "ArrowIPC"]` — a decoded tabular
/// expression ready to use in Wolfram Language without further conversion.
#[export(wxf)]
fn db_query(
    id: String,
    sql: String,
    params: HashMap<String, String>,
) -> Result<Expr, String> {
    let bytes = wolfram_examples::duckdb::query(&id, &sql, params)?;
    let byte_array = Expr::from(bytes);
    Ok(expr!(ImportByteArray[byte_array, "ArrowIPC"]))
}

#[export(wxf)]
fn db_disconnect(id: String) -> Result<String, String> {
    wolfram_examples::duckdb::disconnect(&id)
}
