#!/usr/bin/env wolframscript
(* Generate notebooks/WolframExampleTour.nb - a runnable tour of the
   WolframExample paclet, one section per Rust library.

   Usage:
     wolframscript -file scripts/generate-example-notebook.wls
     wolframscript -file scripts/generate-example-notebook.wls --check
     wolframscript -file scripts/generate-example-notebook.wls --out /tmp/Tour.nb

   The notebook is written unevaluated - open it and press Evaluate Notebook.
   --check additionally evaluates every input cell in this kernel and reports
   the ones that fail, which is how you tell that a tour cell went stale after
   an API change. It needs the library built first:
       cd WolframExample && cargo wl build
   (Outputs are not inserted into the notebook: typesetting Tabular and Dataset
   results properly needs a front end, so the saved file stays input-only.)

   The cell text lives in $sections below: edit there, re-run, done. Keeping it
   in a script rather than hand-editing the .nb means the tour can be
   regenerated whenever the paclet's API changes, and stays reviewable as a
   diff.

   Keep this file 7-bit ASCII. wolframscript does not read .wls source as UTF-8
   by default, so a literal em dash here arrives as its three raw bytes and
   lands in the notebook as \[AHat]\.80\.94. Write \[LongDash] (or any other
   long name) in the cell strings instead \[LongDash] it survives either way. *)

(* -- Arguments --------------------------------------------------------------- *)

$args = Rest[$ScriptCommandLine];
$check = MemberQ[$args, "--check"];
$outArg = Replace[
    FirstPosition[$args, "--out"],
    {{i_Integer} :> $args[[i + 1]], _ :> None}
];

$repoRoot = ParentDirectory[DirectoryName[$InputFileName]];
$out = Replace[$outArg, None :> FileNameJoin[{$repoRoot, "notebooks", "WolframExampleTour.nb"}]];

(* -- Cell constructors ------------------------------------------------------- *)

title[text_] := Cell[text, "Title"];
text[body_] := Cell[body, "Text"];
code[input_] := Cell[input, "Input"];

(* A section is a closed-able group: its heading followed by its cells. *)
section[heading_, cells___] := Cell[CellGroupData[{Cell[heading, "Section"], cells}, Open]];
subsection[heading_, cells___] := Cell[CellGroupData[{Cell[heading, "Subsection"], cells}, Open]];

(* -- The tour ---------------------------------------------------------------- *)

$sections = {

title["WolframExample: Rust libraries from the Wolfram Language"],

text["This notebook exercises the WolframExample paclet, whose functions are \
implemented in Rust (see WolframExample/Libs). Evaluate it top to bottom.

If the setup section reports a missing paclet, build the library first:
    cd WolframExample && cargo wl build"],

section["Setup",

    text["Both paclets \[LongDash] the hand-written WolframExample and the generated \
WolframExampleLib \[LongDash] sit in this notebook's parent directory, so one \
PacletDirectoryLoad registers both."],

    code["PacletDirectoryLoad[ParentDirectory[NotebookDirectory[]]]"],

    code["PacletObject[\"WolframExample\"]"],

    text["The library paclet holds nothing but the compiled dylibs and a \
generated Functions.wl loader:"],

    code["PacletObject[\"WolframExampleLib\"]"],

    code["Needs[\"WolframExample`\"]"],

    text["Everything the package exports:"],

    code["?WolframExample`*"]
],

section["Math",

    subsection["Scalars and arrays",

        text["A plain Rust function over two f64s. Arguments and result cross \
as raw MArgument scalars \[LongDash] no serialization at all."],

        code["ExampleAdd[2, 3]"],

        text["NumericArrays are read zero-copy: Rust sees the kernel's own \
memory, no copy and no conversion. Lists are packed into a Real64 \
NumericArray by the wrapper first."],

        code["ExampleDot[{1, 2, 3}, {4, 5, 6}]"],

        code["ExampleDot[\n    NumericArray[Range[1., 100000.], \"Real64\"],\n    NumericArray[Range[100000., 1., -1.], \"Real64\"]\n]"],

        text["The Rust side accumulates into 8 independent lanes so that LLVM \
can vectorize the loop (see Libs/math/src/lib.rs) \[LongDash] about twice the \
throughput of the naive left-to-right sum. Timed against the kernel's own Dot, \
which calls into BLAS; a Speedup above 1 means the Rust library won.

Two things decide what you see here. Time only the call, not the packing: both \
arrays are converted to NumericArrays first, since passing plain lists spends \
about three times the call itself on the conversion. And build with \
    cargo wl build --release
\[LongDash] cargo wl test leaves an unoptimized build in place, which loses by \
an order of magnitude."],

        code["With[\n    {a = Range[1., 100000.], b = Range[100000., 1., -1.]},\n    {na = NumericArray[a, \"Real64\"], nb = NumericArray[b, \"Real64\"]},\n    {rust = First @ RepeatedTiming @ ExampleDot[na, nb],\n     kernel = First @ RepeatedTiming @ Dot[a, b],\n     packing = First @ RepeatedTiming @ ExampleDot[a, b]},\n    <|\n        \"Rust\" -> Quantity[rust, \"Seconds\"],\n        \"Kernel\" -> Quantity[kernel, \"Seconds\"],\n        \"Speedup\" -> kernel / rust,\n        \"RustFromLists\" -> Quantity[packing, \"Seconds\"]\n    |>\n]"]
    ],

    subsection["Structs and enums",

        text["Structured arguments cross as WXF. A Rust struct is an \
Association keyed by its field names; the wrapper here presents \
Wolfram-style capitalized keys."],

        code["ExampleSymmetricPoint[<|\"X\" -> 10, \"Y\" -> 20|>]"],

        text["ExampleArea goes through a Rust enum \[LongDash] one function, one \
match, two shapes:"],

        code["ExampleArea[<|\"Radius\" -> 1|>]"],

        code["ExampleArea[<|\"Width\" -> 10, \"Height\" -> 20|>]"],

        code["Table[ExampleArea[<|\"Radius\" -> r|>], {r, 1, 5}]"]
    ],

    subsection["Errors",

        text["A Rust Result<f64, String> comes back as {\"Ok\", value} or \
{\"Err\", message}; the wrapper turns the error branch into a Failure."],

        code["ExampleSafeDivide[10, 4]"],

        code["ExampleSafeDivide[1, 0]"],

        text["So the usual Failure handling applies:"],

        code["FailureQ /@ {ExampleSafeDivide[1, 2], ExampleSafeDivide[1, 0]}"]
    ]
],

section["URL encoding",

    text["ExampleURLEncode and ExampleURLDecode are drop-in equivalents of the \
kernel's own URLEncode and URLDecode, built on percent-encoding \[LongDash] the \
RFC 3986 half of the Rust url crate. Same character set, same uppercase hex, \
same treatment of a bare + on the way back."],

    code["ExampleURLEncode[\"Hello, world!\"]"],

    code["ExampleURLDecode[\"Hello%2C%20world%21\"]"],

    subsection["Lists are the real interface",

        text["The Rust functions take a list and return a list, and that is \
all they take: crossing the library boundary costs one WXF round trip, so \
encoding n strings should be one call rather than n. The single-string form \
above is a two-line wrapper in Kernel/Functions/URL.wl \[LongDash] wrap, call, \
unwrap."],

        code["ExampleURLEncode[{\"a+b\", \"7 x + 5 y\", \"Kurt G\\[ODoubleDot]del\"}]"],

        text["Decoding is deliberately asymmetric, exactly as the kernel's is: \
a literal + is a space, while %2B is a plus sign. Anything that is not a \
complete escape is left alone."],

        code["ExampleURLDecode[{\"a+b\", \"a%2Bb\", \"a+b%2Bc\", \"100%\", \"%zz\"}]"]
    ],

    subsection["Agreement with the kernel",

        text["Not a spot check: every character in the Basic Multilingual \
Plane, encoded both ways and compared. Lone surrogates are left out because \
they have no UTF-8 form at all \[LongDash] a Wolfram string can hold one, a Rust \
str cannot, and the library boundary rejects the payload before any of this \
code runs."],

        code["bmp = FromCharacterCode /@ Join[Range[0, 16^^D7FF], Range[16^^E000, 2^16 - 1]];\nLength[bmp]"],

        code["Counts[MapThread[SameQ, {ExampleURLEncode[bmp], URLEncode /@ bmp}]]"],

        code["Counts[MapThread[SameQ, {ExampleURLDecode[URLEncode /@ bmp], bmp}]]"]
    ],

    subsection["Benchmark",

        text["Both directions, against the kernel's own implementation, over \
lists of 40-character strings drawn from an alphabet that is mostly \
punctuation and non-ASCII \[LongDash] so almost every character needs escaping. \
Speedup above 1 means the Rust library won.

At n = 1 the WXF round trip is most of the call and the margin is small; by \
n = 100 it has amortized away and the ratio settles. As with ExampleDot, build \
with\n    cargo wl build --release\n\[LongDash] cargo wl test leaves an \
unoptimized build in place."],

        code["SeedRandom[42];\nalphabet = Join[Characters[\"abcdefgh ?&=/:%\"], {\"\\[ODoubleDot]\", \"\\[Alpha]\"}];\nsample[n_] := Table[StringJoin[RandomChoice[alphabet, 40]], n];"],

        code["row[n_] := Module[{data = sample[n], encoded},\n    encoded = URLEncode /@ data;\n    <|\n        \"n\" -> n,\n        \"EncodeSpeedup\" -> First[RepeatedTiming[URLEncode /@ data]] /\n                           First[RepeatedTiming[ExampleURLEncode[data]]],\n        \"DecodeSpeedup\" -> First[RepeatedTiming[URLDecode /@ encoded]] /\n                           First[RepeatedTiming[ExampleURLDecode[encoded]]]\n    |>\n];\nDataset[row /@ {1, 10, 100, 1000, 10000}]"]
    ]
],

section["DuckDB",

    text["An entire embedded SQL engine, statically linked into the library \[LongDash] \
nothing to install. Query results come back as Arrow and are decoded into a \
Tabular."],

    subsection["Scoped connections",

        text["DuckDBExecute opens a connection, applies the function to it, \
and closes it again \[LongDash] even if the body fails."],

        code["DuckDBExecute[\"duckdb://\", DuckDBQuery[#, \"SELECT 42 AS answer\"] &]"],

        code["DuckDBExecute[\"duckdb://\", DuckDBQuery[#,\n    \"SELECT n::INTEGER AS id,\n            (n * 1.5)::DOUBLE AS score,\n            'item_' || n AS label,\n            n % 2 = 0 AS is_even\n     FROM generate_series(1, 5) t(n)\"] &]"],

        text["The result is an ordinary Tabular, so the usual operations work:"],

        code["tab = DuckDBExecute[\"duckdb://\", DuckDBQuery[#,\n    \"SELECT n FROM generate_series(1, 100) t(n)\"] &];\n{Length[tab], Total[tab[[All, \"n\"]]]}"]
    ],

    subsection["Parameters",

        text["Values are bound to the ? placeholders in order, so nothing is \
interpolated into the SQL string."],

        code["DuckDBExecute[\"duckdb://\", DuckDBQuery[#,\n    \"SELECT n FROM generate_series(1, 5) t(n) WHERE n > ?\", {2}] &]"],

        code["DuckDBExecute[\"duckdb://\", DuckDBQuery[#,\n    \"SELECT ?::INTEGER + ?::INTEGER AS total\", {20, 22}] &]"]
    ],

    subsection["Explicit connections",

        text["For several statements against the same database, hold the \
handle yourself. Here an on-disk file, which persists after disconnecting."],

        code["file = FileNameJoin[{$TemporaryDirectory, \"wolfram-example.duckdb\"}];\nQuiet[DeleteFile[file]];\nconn = DuckDBConnect[\"duckdb:///\" <> file]"],

        code["DuckDBQuery[conn,\n    \"CREATE TABLE fruit AS SELECT * FROM (VALUES (1, 'apple'), (2, 'pear')) v(id, name)\"]"],

        code["DuckDBQuery[conn, \"INSERT INTO fruit VALUES (?, ?)\", {3, \"plum\"}]"],

        code["DuckDBQuery[conn, \"SELECT * FROM fruit ORDER BY id\"]"],

        code["DuckDBDisconnect[conn]"],

        text["Reopening the same file shows the rows are still there:"],

        code["DuckDBExecute[\"duckdb:///\" <> file,\n    DuckDBQuery[#, \"SELECT count(*) AS rows FROM fruit\"] &]"]
    ],

    subsection["Failures",

        text["Errors from the engine arrive as Failure objects carrying \
DuckDB's own message, and propagate on their own."],

        code["DuckDBExecute[\"duckdb://\", DuckDBQuery[#, \"SELECT * FROM no_such_table\"] &]"],

        text["A handle that has been closed is no longer in the Rust-side \
registry:"],

        code["With[{c = DuckDBConnect[]},\n    DuckDBDisconnect[c];\n    DuckDBQuery[c, \"SELECT 1\"]\n]"]
    ]
],

section["The raw library",

    text["The wrappers in Kernel/WolframExample.wl are a thin layer over an \
association of LibraryFunctions, keyed \"namespace::rust_name\". Anything \
without a wrapper is still one lookup away."],

    code["Keys[WolframExampleFunctions[]]"],

    text["inactive_sum builds an expression in Rust and returns it \
unevaluated:"],

    code["WolframExampleFunctions[][\"math::inactive_sum\"][{1, 2, 3}]"],

    code["Activate[WolframExampleFunctions[][\"math::inactive_sum\"][{1, 2, 3}]]"],

    text["A panic on the Rust side is caught at the boundary and returned as \
a Failure \[LongDash] it does not take the kernel down:"],

    code["WolframExampleFunctions[][\"math::panic\"][]"],

    text["And the signatures of everything that was packaged, straight from \
the generated Artifacts.wl asset:"],

    code["Get[PacletObject[\"WolframExampleLib\"][\"AssetLocation\", \"Artifacts\"]] //\n    Map[Query[{\"Name\", \"Signatures\"}]] // Dataset"]
],

section["Licensing",

    text["Everything the library links is compiled into the dylib, so the \
paclet has to carry its own attribution. cargo wl build writes that out as a \
third asset, License.wl: one entry per Rust package in the resolved dependency \
graph, dev-dependencies excluded."],

    code["licenses = Get[PacletObject[\"WolframExampleLib\"][\"AssetLocation\", \"License\"]];\nLength[licenses]"],

    text["Each entry carries what the crate's own Cargo.toml declared \
\[LongDash] undeclared fields come back as Missing[\"NotAvailable\"]:"],

    code["Dataset[Take[licenses, 8]]"],

    text["Which licenses the paclet actually ships under, most common first. \
The values are SPDX expressions, so an OR means the crate lets you pick:"],

    code["ReverseSort[Counts[licenses[[All, \"License\"]]]] // Dataset"],

    text["The entries worth a human's attention \[LongDash] anything that \
declared no license at all. Here that is the two example crates themselves: \
neither Libs/math nor Libs/duckdb sets a license field, which a paclet you \
actually shipped would want to fix:"],

    code["Select[licenses, MissingQ[#[\"License\"]] &] // Dataset"],

    text["And the direct dependencies of the two example crates, as opposed to \
the whole transitive graph:"],

    code["Select[licenses,\n    MemberQ[{\"duckdb\", \"arrow-ipc\", \"uuid\", \"wolfram-expr\",\n             \"wolfram-export\", \"wolfram-serialize\", \"wolfram-library-link\"},\n        #[\"Name\"]] &] // Dataset"]
],

section["Rebuilding",

    text["After editing the Rust source, rebuild from the paclet directory. \
The dylibs are content-hashed, so a rebuilt library gets a new file name \[LongDash] \
quit the kernel to pick it up."],

    code["ExternalEvaluate[{\"Shell\",\n    \"ProcessDirectory\" -> FileNameJoin[{ParentDirectory[NotebookDirectory[]], \"WolframExample\"}],\n    \"ProcessEnvironment\" -> <|\"PATH\" -> Environment[\"PATH\"]|>},\n   \"cargo wl build --release\"]"]
]

};

(* -- Write ------------------------------------------------------------------- *)

$notebook = Notebook[$sections,
    StyleDefinitions -> "Default.nb",
    WindowTitle -> "WolframExample Tour"
];

If[!DirectoryQ[DirectoryName[$out]], CreateDirectory[DirectoryName[$out], CreateIntermediateDirectories -> True]];

Export[$out, $notebook, "NB"];
Print["wrote ", $out];

(* -- --check: evaluate every input cell here in the kernel ------------------- *)

(* NotebookDirectory[] only exists with a front end, so the cells that call it
   are evaluated against the real notebook location instead. *)
(* the setup section: PacletDirectoryLoad, two PacletObject lookups, Needs *)
$setupCells = 4;

$inputs = Cases[$notebook, Cell[input_String, "Input"] :> input, Infinity];

label[input_String] := First[StringSplit[input, "\n"]];

(* A cell counts as failed when it emits a message (Check turns that into
   $Failed). A Failure *object* is not a failure of the cell: several cells
   demonstrate error handling and are meant to return one. *)
(* The rebuild cell shells out to cargo - checked cells must not have side
   effects outside the kernel, so it is skipped before it is ever evaluated. *)
checkCell[input_String] /; StringContainsQ[input, "ExternalEvaluate"] := (
    Print["skip ", label[input]];
    True
);

checkCell[input_String] := With[
    {code = StringReplace[input, "NotebookDirectory[]" -> ToString[DirectoryName[$out], InputForm]]},
    {result = Check[ToExpression[code], $Failed]},
    Which[
        result === $Failed,
            Print["FAIL ", label[input]]; False,
        FailureQ[result],
            Print["ok   ", label[input], "  (Failure, as intended)"]; True,
        True,
            Print["ok   ", label[input]]; True
    ]
];

If[$check,
    Print["checking ", Length[$inputs], " input cells..."];
    (* The first cells register and load the paclet. If that did not take,
       every later cell would sit there unevaluated and still look fine, so
       stop with a real explanation instead. *)
    checkCell /@ Take[$inputs, $setupCells];
    If[!MemberQ[$Packages, "WolframExample`"],
        Print["cannot check: WolframExample` did not load from ", DirectoryName[$out]];
        Print["build the library first (cd WolframExample && cargo wl build), and"];
        Print["check the notebook where it will live \[LongDash] --out must stay next to the paclet"];
        Exit[1]
    ];
    With[{failed = Count[checkCell /@ Drop[$inputs, $setupCells], False]},
        Print[If[failed == 0, "all cells ok", ToString[failed] <> " cell(s) failed"]];
        If[failed > 0, Exit[1]]
    ]
];
