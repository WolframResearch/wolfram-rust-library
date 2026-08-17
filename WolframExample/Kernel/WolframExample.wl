(* ::Package:: *)

(* Top-level Wolfram Language API for the Rust libraries in ../Libs.

   The Rust side is packaged by `cargo wl build` into a *separate* paclet,
   WolframExampleLib, which contains nothing but the compiled dylibs and a
   generated Functions.wl. That file evaluates to an association of ready-made
   LibraryFunction objects keyed by "<namespace>::<rust_fn_name>", e.g.
   "duckdb::db_query". Everything below is hand-written boilerplate that turns
   those raw handles into named, documented, argument-checked WL functions. *)

BeginPackage["WolframExample`"];

WolframExampleFunctions::usage = "WolframExampleFunctions[] gives the association of raw LibraryFunction objects exported by the WolframExampleLib paclet, keyed by \"namespace::name\".";

(* math *)
ExampleAdd::usage = "ExampleAdd[a, b] adds two reals in Rust.";
ExampleDot::usage = "ExampleDot[a, b] gives the dot product of two packed Real64 NumericArrays, read zero-copy by Rust.";
ExampleArea::usage = "ExampleArea[<|\"Radius\" -> r|>] or ExampleArea[<|\"Width\" -> w, \"Height\" -> h|>] gives the area of a circle or rectangle, dispatched through a Rust enum.";
ExampleSymmetricPoint::usage = "ExampleSymmetricPoint[<|\"X\" -> x, \"Y\" -> y|>] reflects a point through the origin.";
ExampleSafeDivide::usage = "ExampleSafeDivide[a, b] gives a/b, or a Failure when b is zero.";

(* duckdb *)
DuckDBConnect::usage = "DuckDBConnect[] opens an in-memory DuckDB connection and gives its handle. DuckDBConnect[url] opens \"duckdb:///path/to/file.db\", or ATTACHes a \"postgres://\", \"sqlite://\" or \"mysql://\" database.";
DuckDBQuery::usage = "DuckDBQuery[conn, sql] runs sql and gives the result as a Tabular. DuckDBQuery[conn, sql, {v1, v2, ...}] binds the values to the ? placeholders in sql, in order.";
DuckDBDisconnect::usage = "DuckDBDisconnect[conn] closes the connection.";
DuckDBExecute::usage = "DuckDBExecute[url, f] opens a connection to url, applies f to it, closes it again, and gives the result of f — even if f fails.";

(* No backticks around the shell command: in a message template a backtick
   pair is a slot marker, so "`cargo wl build`" would be read as slot 0. *)
WolframExample::nolib = "The `1` paclet was not found. Build it by running \"cargo wl build\" in WolframExample, then make its directory known with PacletDirectoryLoad.";
WolframExample::nofunc = "`1` is not exported by the `2` paclet; the library may be built without the corresponding feature.";
WolframExample::divide = "`1`";
WolframExample::badasset = "The `1` paclet does not provide a readable \"Functions\" asset (looked at `2`).";

Begin["`Private`"];

$libraryPaclet = "WolframExampleLib";

(* ── Loading ────────────────────────────────────────────────────────────────
   PacletObject[name]["AssetLocation", "Functions"] resolves the "Functions"
   asset that the generated PacletInfo.wl declares, so we never hard-code a
   path or a SystemID directory: the paclet manager picks the build matching
   this machine. (The property is singular — "AssetsLocation" is not one of
   PacletObject's properties and evaluates to an unevaluated part access.) *)

(* Each With sequence sees the previous one's bindings, so the three stages —
   find the paclet, resolve the asset, read it — chain without any mutable
   state; every stage guards on the one before it, and a single Which reports
   whichever one gave out. *)
loadFunctions[] := With[
    {paclet = PacletObject[$libraryPaclet]},
    {file = If[PacletObjectQ[paclet], paclet["AssetLocation", "Functions"], $Failed]},
    {functions = If[StringQ[file] && FileExistsQ[file], Get[file], $Failed]},
    Which[
        !PacletObjectQ[paclet],
            Message[WolframExample::nolib, $libraryPaclet]; $Failed,
        !StringQ[file] || !FileExistsQ[file],
            Message[WolframExample::badasset, $libraryPaclet, file]; $Failed,
        !AssociationQ[functions],
            Message[WolframExample::badasset, $libraryPaclet, file]; $Failed,
        True,
            functions
    ]
];

(* Cache only a successful load, so a failed lookup can be retried after the
   library has been built or PacletDirectoryLoad-ed. *)
$functions := With[{loaded = loadFunctions[]},
    If[AssociationQ[loaded], $functions = loaded, loaded]
];

WolframExampleFunctions[] := Replace[$functions, Except[_?AssociationQ] :> <||>];

(* `libraryFunction["duckdb::db_query"][args]` — resolves lazily on each call
   and messages once if the key is absent (e.g. a Cargo feature was off when
   the library was built). *)
libraryFunction[key_String][args___] := With[
    {functions = $functions},
    {f = If[AssociationQ[functions], Lookup[functions, key, $Failed], $Failed]},
    Which[
        (* the load already messaged — stay quiet here *)
        !AssociationQ[functions], $Failed,
        f === $Failed, Message[WolframExample::nofunc, key, $libraryPaclet]; $Failed,
        True, f[args]
    ]
];

(* ── math ─────────────────────────────────────────────────────────────────── *)

ExampleAdd[a_?NumericQ, b_?NumericQ] := libraryFunction["math::add"][N[a], N[b]];

(* The Rust side takes &NumericArray<f64>, so both arguments must be Real64
   NumericArrays; plain lists are converted here rather than in the library. *)
ExampleDot[a_List, b_List] := ExampleDot[toReal64[a], toReal64[b]];
ExampleDot[a_NumericArray, b_NumericArray] :=
    libraryFunction["math::dot"][a, b];

toReal64[list_List] := NumericArray[N[list], "Real64"];

(* One Rust function, two shapes: `area_shape` takes the `Shape` enum, whose
   WL form is a two-element {"VariantName", <|fields|>} — any head works, so
   "Circle" -> <|"radius" -> 1|> would do just as well. Struct fields keep
   their Rust names ("width", "radius", ...). *)
ExampleArea[assoc_?AssociationQ] := With[{shape = shapeFor[assoc]},
    If[shape === $Failed, $Failed, libraryFunction["math::area_shape"][shape]]
];

shapeFor[assoc_] := Which[
    KeyExistsQ[assoc, "Radius"],
        {"Circle", <|"radius" -> N[assoc["Radius"]]|>},
    KeyExistsQ[assoc, "Width"] && KeyExistsQ[assoc, "Height"],
        {"Rect", <|"width" -> N[assoc["Width"]], "height" -> N[assoc["Height"]]|>},
    True,
        $Failed
];

ExampleSymmetricPoint[assoc_?AssociationQ] := With[
    {point = libraryFunction["math::symmetric_point"][
        <|"x" -> N[assoc["X"]], "y" -> N[assoc["Y"]]|>
    ]},
    If[AssociationQ[point], <|"X" -> point["x"], "Y" -> point["y"]|>, point]
];

(* A Rust `Result<f64, String>` crosses as the plain enum {"Ok", value} /
   {"Err", message} — only error types deriving `Failure` come back as a real
   Failure — so the wrapper is where that becomes idiomatic WL. *)
ExampleSafeDivide[a_?NumericQ, b_?NumericQ] := Replace[
    libraryFunction["math::safe_divide"][N[a], N[b]],
    {
        {"Ok", value_} :> value,
        {"Err", message_} :> Failure["DivisionError", <|
            "MessageTemplate" :> WolframExample::divide,
            "MessageParameters" -> {message},
            "Message" -> message
        |>]
    }
];

(* ── duckdb ───────────────────────────────────────────────────────────────────
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

End[];

EndPackage[];
