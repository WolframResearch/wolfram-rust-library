(* ::Package:: *)

(* Top-level Wolfram Language API for the Rust libraries in ../Libs.

   The Rust side is packaged by `cargo wl build` into a *separate* paclet,
   WolframExampleLib, which contains nothing but the compiled dylibs and a
   generated Functions.wl. That file evaluates to an association of ready-made
   LibraryFunction objects keyed by "<namespace>::<rust_fn_name>", e.g.
   "duckdb::db_query". Everything under Kernel/ is hand-written boilerplate
   that turns those raw handles into named, documented, argument-checked WL
   functions.

   This file is the only one the paclet manager loads (PacletInfo.wl names the
   context, and Root -> "Kernel"). It declares the public symbols, then Gets
   the implementation files. One file per Rust library, so the WL side is split
   the same way Libs/ is. *)

BeginPackage["WolframExample`"];

(* Developer maintains these declarations: a symbol has to be mentioned in this
   file, outside `Private`, or the definitions loaded below would land on a
   private symbol of the same name instead. *)

WolframExampleFunctions::usage = "WolframExampleFunctions[] gives the association of raw LibraryFunction objects exported by the WolframExampleLib paclet, keyed by \"namespace::name\", or a Failure if that paclet cannot be loaded.";

(* math — Functions/Math.wl *)
ExampleAdd::usage = "ExampleAdd[a, b] adds two reals in Rust.";
ExampleDot::usage = "ExampleDot[a, b] gives the dot product of two packed Real64 NumericArrays, read zero-copy by Rust.";
ExampleArea::usage = "ExampleArea[<|\"Radius\" -> r|>] or ExampleArea[<|\"Width\" -> w, \"Height\" -> h|>] gives the area of a circle or rectangle, dispatched through a Rust enum.";
ExampleSymmetricPoint::usage = "ExampleSymmetricPoint[<|\"X\" -> x, \"Y\" -> y|>] reflects a point through the origin.";
ExampleSafeDivide::usage = "ExampleSafeDivide[a, b] gives a/b, or a Failure when b is zero.";

(* url — Functions/URL.wl *)
ExampleURLEncode::usage = "ExampleURLEncode[\"string\"] percent-encodes a string in Rust, matching System`URLEncode. ExampleURLEncode[{s1, s2, ...}] encodes a whole list in a single library call.";
ExampleURLDecode::usage = "ExampleURLDecode[\"string\"] percent-decodes a string in Rust, matching System`URLDecode. ExampleURLDecode[{s1, s2, ...}] decodes a whole list in a single library call.";

(* duckdb — Functions/DuckDB.wl *)
DuckDBConnect::usage = "DuckDBConnect[] opens an in-memory DuckDB connection and gives its handle. DuckDBConnect[url] opens \"duckdb:///path/to/file.db\", or ATTACHes a \"postgres://\", \"sqlite://\" or \"mysql://\" database.";
DuckDBQuery::usage = "DuckDBQuery[conn, sql] runs sql and gives the result as a Tabular. DuckDBQuery[conn, sql, {v1, v2, ...}] binds the values to the ? placeholders in sql, in order.";
DuckDBDisconnect::usage = "DuckDBDisconnect[conn] closes the connection.";
DuckDBExecute::usage = "DuckDBExecute[url, f] opens a connection to url, applies f to it, closes it again, and gives the result of f — even if f fails.";

(* No backticks around the shell command: in a message template a backtick
   pair is a slot marker, so "`cargo wl build`" would be read as slot 0. *)
WolframExample::nolib = "The `1` paclet was not found. Build it by running \"cargo wl build\" in WolframExample, then make its directory known with PacletDirectoryLoad.";
WolframExample::nofunc = "`1` is not exported by the `2` paclet; the library may be built without the corresponding feature.";
WolframExample::divide = "`1`";
WolframExample::shape = "An area needs either a \"Radius\" or a \"Width\" and a \"Height\"; got `1`.";
WolframExample::badasset = "The `1` paclet does not provide a readable \"Functions\" asset (looked at `2`).";

Begin["`Private`"];

(* The loader. Code/ first — it defines libraryFunction[], which every file in
   Functions/ calls — then one file per Rust library, in whatever order
   FileNames gives them: the wrappers are independent of each other.

   The implementation files deliberately have no BeginPackage of their own.
   They are read inside the scope opened above, so their definitions land in
   WolframExample` (public symbols, declared above) or in
   WolframExample`Private` (everything else) with nothing to keep in sync.

   $InputFileName is bound before the Scan because Get rebinds it while each
   file is being read. *)

With[{root = DirectoryName[$InputFileName]},
    Scan[
        Scan[Get, FileNames["*.wl", FileNameJoin[{root, #}]]] &,
        {"Code", "Functions"}
    ]
];

End[];

EndPackage[];
