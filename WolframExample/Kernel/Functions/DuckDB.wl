(* Wrappers for Libs/duckdb — namespace "duckdb".

   Every db_* function returns a transparent success value (the handle string,
   or the Tabular) or a Failure["ConnectionError"/"ExecuteError"/..., <|...|>],
   so results are passed through unchanged — Failures propagate on their own. *)

DuckDBConnect[] := DuckDBConnect["duckdb://"];
DuckDBConnect[url_String] := libraryFunction["duckdb::db_connect"][url];

DuckDBQuery[conn_String, sql_String] := DuckDBQuery[conn, sql, {}];

(* The Rust signature binds a HashMap<String, String> positionally, sorted by
   key — so a user-facing ordered list of values is zero-padded into keys that
   sort in that same order ("01", "02", ... beyond nine params). *)
DuckDBQuery[conn_String, sql_String, params_List] :=
    libraryFunction["duckdb::db_query"][conn, sql, positionalParams[params]];

DuckDBQuery[conn_String, sql_String, params_?AssociationQ] :=
    libraryFunction["duckdb::db_query"][conn, sql, ToString /@ params];

positionalParams[params_List] := With[{width = Max[1, IntegerLength[Length[params]]]},
    AssociationThread[
        IntegerString[Range[Length[params]], 10, width],
        (* Strings bind as-is; anything else goes through InputForm, which
           DuckDB then casts per the column type. *)
        Replace[params, x : Except[_String] :> ToString[x, InputForm], {1}]
    ]
];

DuckDBDisconnect[conn_String] := libraryFunction["duckdb::db_disconnect"][conn];

(* WithCleanup closes the handle even if f aborts or throws, so a failing
   query can't leak a connection out of the registry Rust keeps. *)
DuckDBExecute[url_String, f_] := With[{conn = DuckDBConnect[url]},
    If[StringQ[conn], WithCleanup[f[conn], DuckDBDisconnect[conn]], conn]
];
