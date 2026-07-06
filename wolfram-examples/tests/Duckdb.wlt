(* DuckDB example end-to-end tests.
   Run via: cargo wl test --features duckdb  (from wolfram-examples/)
   DuckDB is statically compiled in — no driver install needed.

   Every db_* function returns one `DuckDbResult` variant (per-variant `enum_head`):
     db_connect    -> id     (transparent: enum_head=false → the uuid string directly)
     db_query      -> table  (transparent: enum_head=false → ImportByteArray[…] directly)
     db_disconnect -> id     (transparent: enum_head=false → the uuid string directly)
     failures      -> Failure["ConnectionError"/"QueryError"/"UnknownConnection", <|…|>]
   The transparent connect / query / disconnect results need no unwrapping —
   they return the id / table directly. *)

$Libs = Quiet[Get["Functions.wl"]];

If[!AssociationQ[$Libs],
    Print["SKIP: could not load Functions.wl"]; Return[]
];
If[!KeyExistsQ[$Libs, "duckdb::db_connect"],
    Print["SKIP: duckdb::db_connect not found (was the duckdb feature enabled?)"];
    Return[]
];

$Connect    = $Libs["duckdb::db_connect"];
$Query      = $Libs["duckdb::db_query"];
$Disconnect = $Libs["duckdb::db_disconnect"];

$Id = $Connect["duckdb://"];

$Tests = {

    (* scalar SELECT — query is transparent, so the table comes back directly *)
    <|"TestID"   -> "DuckDB-scalar",
      "Input"    -> First[Values[Normal[$Query[$Id, "SELECT 42 AS x", <||>]]]][[1]],
      "Output"   -> 42,
      "Messages" -> {}|>,

    (* DuckDB generate_series — int, float, string, bool in one query.
       Normal[] on a Tabular is row-oriented: a list of per-row associations,
       so Keys[Normal[...]] gives one column-name list per row, not a single
       flat list. *)
    <|"TestID"   -> "DuckDB-mixed-types",
      "Input"    -> Keys[Normal[$Query[$Id,
                        "SELECT n::INTEGER AS id,
                                (n * 1.5)::DOUBLE AS score,
                                'item_' || n       AS label,
                                n % 2 = 0          AS is_even
                         FROM generate_series(1, 3) t(n)", <||>]]],
      "Output"   -> ConstantArray[{"id", "score", "label", "is_even"}, 3],
      "Messages" -> {}|>,

    (* row count — Normal[] on a Tabular gives one row-association per row,
       so Length[Normal[...]] is the row count directly. *)
    <|"TestID"   -> "DuckDB-row-count",
      "Input"    -> Length[Normal[$Query[$Id,
                        "SELECT n FROM generate_series(1, 5) t(n)", <||>]]],
      "Output"   -> 5,
      "Messages" -> {}|>,

    (* db_disconnect is transparent — the id comes back directly, no wrapper head *)
    <|"TestID"   -> "DuckDB-disconnect-returns-id",
      "Input"    -> Head[$Disconnect[$Connect["duckdb://"]]],
      "Output"   -> String,
      "Messages" -> {}|>,

    (* bad SQL → Failure["QueryError", <|"Message" -> …|>] *)
    <|"TestID"   -> "DuckDB-bad-sql",
      "Input"    -> $Query[$Id, "SELECT * FROM no_such_table", <||>],
      "Output"   -> _Failure,
      "Messages" -> {}|>,

    (* querying a closed handle → Failure["UnknownConnection", …] *)
    <|"TestID"   -> "DuckDB-unknown-connection",
      "Input"    -> Module[{tmp = $Connect["duckdb://"]},
                       $Disconnect[tmp];
                       $Query[tmp, "SELECT 1", <||>]],
      "Output"   -> _Failure,
      "Messages" -> {}|>,

    (* disconnect returns the id directly — run last *)
    <|"TestID"   -> "DuckDB-disconnect",
      "Input"    -> $Disconnect[$Id],
      "Output"   -> $Id,
      "Messages" -> {}|>

};

TestCreate[
    #Input,
    #Output,
    #Messages,
    SameTest -> MatchQ,
    TestID -> #TestID
] & /@ $Tests
