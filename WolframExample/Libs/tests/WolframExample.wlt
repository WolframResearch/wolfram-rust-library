(* End-to-end test of the WolframExample paclet's top-level API.
   Run via: cargo wl test  (from WolframExample/)

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

(* Shared corpora for the url tests below. *)
$URLCorpus = {"", "a+b", "Hello, world!", "7 x + 5 y", "123.123123",
    "abcdef!@#$%^&*()", "http://www.wolfram.com/solutions", "\n",
    "Kurt G\[ODoubleDot]del", "Paul Erd\[ODoubleAcute]s", "-_.~"};

$URLBMP = FromCharacterCode /@ Join[Range[0, 16^^D7FF], Range[16^^E000, 2^16 - 1]];

$URLDecodeLiterals = {"a+b", "a+b%2Bc", "100%", "%zz", "%2", "adfad\[EGrave]", "",
    "%C3%B6", "%c3%b6"};

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

    (* an unrecognized shape is reported as a Failure, not a bare $Failed *)
    <|"TestID"   -> "WolframExample-area-unknown-shape",
      "Input"    -> WolframExample`ExampleArea[<|"Bogus" -> 1|>],
      "Output"   -> _Failure,
      "Messages" -> {}|>,

    (* the loaded library is handed back as-is *)
    <|"TestID"   -> "WolframExample-functions-association",
      "Input"    -> AssociationQ[WolframExample`WolframExampleFunctions[]],
      "Output"   -> True,
      "Messages" -> {}|>,

    (* ── url ──────────────────────────────────────────────────────────────────
       The point of these is agreement with the kernel's own URLEncode /
       URLDecode, so the expected values are computed by System` rather than
       written out: a divergence shows up as a failure here, not as a stale
       literal someone has to re-derive. *)

    (* the corpus from URLUtilities' own URLEncode.mt, character for character *)
    <|"TestID"   -> "WolframExample-url-encode-corpus",
      "Input"    -> WolframExample`ExampleURLEncode[$URLCorpus],
      "Output"   -> URLEncode /@ $URLCorpus,
      "Messages" -> {}|>,

    <|"TestID"   -> "WolframExample-url-decode-corpus",
      "Input"    -> WolframExample`ExampleURLDecode[URLEncode /@ $URLCorpus],
      "Output"   -> $URLCorpus,
      "Messages" -> {}|>,

    (* every non-surrogate BMP character. Lone surrogates are excluded on
       purpose: they have no UTF-8 form, so the library boundary rejects the
       payload before Rust sees it (see Libs/url/src/lib.rs). *)
    <|"TestID"   -> "WolframExample-url-encode-bmp",
      "Input"    -> WolframExample`ExampleURLEncode[$URLBMP],
      "Output"   -> URLEncode /@ $URLBMP,
      "Messages" -> {}|>,

    <|"TestID"   -> "WolframExample-url-decode-bmp",
      "Input"    -> WolframExample`ExampleURLDecode[URLEncode /@ $URLBMP],
      "Output"   -> $URLBMP,
      "Messages" -> {}|>,

    (* the decoder's awkward corners: a bare + is a space while %2B is a plus,
       and anything that isn't a complete escape is left alone *)
    <|"TestID"   -> "WolframExample-url-decode-literals",
      "Input"    -> WolframExample`ExampleURLDecode[$URLDecodeLiterals],
      "Output"   -> URLDecode /@ $URLDecodeLiterals,
      "Messages" -> {}|>,

    (* the scalar form is the list form with the list taken off again *)
    <|"TestID"   -> "WolframExample-url-scalar",
      "Input"    -> {WolframExample`ExampleURLEncode["Hello, world!"],
                     WolframExample`ExampleURLDecode["Hello%2C%20world%21"]},
      "Output"   -> {"Hello%2C%20world%21", "Hello, world!"},
      "Messages" -> {}|>,

    (* list in, list out — including the empty one *)
    <|"TestID"   -> "WolframExample-url-empty-list",
      "Input"    -> {WolframExample`ExampleURLEncode[{}], WolframExample`ExampleURLDecode[{}]},
      "Output"   -> {{}, {}},
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
