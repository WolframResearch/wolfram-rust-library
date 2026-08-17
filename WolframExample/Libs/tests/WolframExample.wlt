(* End-to-end test of the WolframExample paclet's top-level API.
   Run via: cargo wl test  (from WolframExample/Libs/)

   Unlike Duckdb.wlt — which loads Functions.wl directly and calls the raw
   LibraryFunctions — this exercises the whole chain: the generated
   WolframExampleLib paclet is discovered by the paclet manager, and
   WolframExample`'s kernel code resolves it via
   PacletObject["WolframExampleLib"]["AssetLocation", "Functions"].

   cargo wl test runs with the generated WolframExampleLib-<SystemID>/ as the
   current directory, and that directory sits next to the WolframExample paclet
   (see `output` in ../math/Cargo.toml), so one PacletDirectoryLoad of the
   parent registers both. *)

PacletDirectoryLoad[ParentDirectory[]];

$loaded = Quiet[Needs["WolframExample`"]] =!= $Failed &&
          PacletObjectQ[PacletObject["WolframExampleLib"]];

If[!$loaded,
    Print["SKIP: WolframExample` / WolframExampleLib not loadable from ",
        ParentDirectory[]];
    Return[]
];

$Tests = {

    (* ── math: native MArgument functions ──────────────────────────────────── *)

    <|"TestID"   -> "WolframExample-add",
      "Input"    -> WolframExample`ExampleAdd[2, 3],
      "Output"   -> 5.,
      "Messages" -> {}|>,

    (* lists are converted to Real64 NumericArrays by the kernel wrapper *)
    <|"TestID"   -> "WolframExample-dot",
      "Input"    -> WolframExample`ExampleDot[{1, 2, 3}, {4, 5, 6}],
      "Output"   -> 32.,
      "Messages" -> {}|>,

    (* ── math: WXF structs and enums ───────────────────────────────────────── *)

    <|"TestID"   -> "WolframExample-area-circle",
      "Input"    -> Round[WolframExample`ExampleArea[<|"Radius" -> 1|>], 10^-6],
      "Output"   -> Round[Pi, 10^-6],
      "Messages" -> {}|>,

    <|"TestID"   -> "WolframExample-area-rect",
      "Input"    -> WolframExample`ExampleArea[<|"Width" -> 10, "Height" -> 20|>],
      "Output"   -> 200.,
      "Messages" -> {}|>,

    <|"TestID"   -> "WolframExample-symmetric-point",
      "Input"    -> WolframExample`ExampleSymmetricPoint[<|"X" -> 10, "Y" -> 20|>],
      "Output"   -> <|"X" -> -10., "Y" -> -20.|>,
      "Messages" -> {}|>,

    <|"TestID"   -> "WolframExample-safe-divide",
      "Input"    -> WolframExample`ExampleSafeDivide[10, 4],
      "Output"   -> 2.5,
      "Messages" -> {}|>,

    (* Err(String) from Rust surfaces as a Failure *)
    <|"TestID"   -> "WolframExample-safe-divide-by-zero",
      "Input"    -> WolframExample`ExampleSafeDivide[1, 0],
      "Output"   -> _Failure,
      "Messages" -> {}|>,

    (* ── duckdb ────────────────────────────────────────────────────────────── *)

    (* DuckDBExecute opens, runs, and closes; the query result is a Tabular,
       and Normal on it is row-oriented. *)
    <|"TestID"   -> "WolframExample-duckdb-scalar",
      "Input"    -> First[Values[Normal[
                        WolframExample`DuckDBExecute["duckdb://",
                            WolframExample`DuckDBQuery[#, "SELECT 42 AS x"] &]]]][[1]],
      "Output"   -> 42,
      "Messages" -> {}|>,

    (* positional params: bound to the ? placeholders in order *)
    <|"TestID"   -> "WolframExample-duckdb-params",
      "Input"    -> Length[Normal[
                        WolframExample`DuckDBExecute["duckdb://",
                            WolframExample`DuckDBQuery[#,
                                "SELECT n FROM generate_series(1, 5) t(n) WHERE n > ?",
                                {2}] &]]],
      "Output"   -> 3,
      "Messages" -> {}|>,

    (* bad SQL propagates the Rust-side Failure through the wrapper *)
    <|"TestID"   -> "WolframExample-duckdb-bad-sql",
      "Input"    -> WolframExample`DuckDBExecute["duckdb://",
                        WolframExample`DuckDBQuery[#, "SELECT * FROM no_such_table"] &],
      "Output"   -> _Failure,
      "Messages" -> {}|>,

    (* connect / disconnect round-trip through the handle registry *)
    <|"TestID"   -> "WolframExample-duckdb-connect-disconnect",
      "Input"    -> Module[{conn = WolframExample`DuckDBConnect[]},
                        WolframExample`DuckDBDisconnect[conn] === conn],
      "Output"   -> True,
      "Messages" -> {}|>

};

TestCreate[
    #Input,
    #Output,
    #Messages,
    SameTest -> MatchQ,
    TestID -> #TestID
] & /@ $Tests
