# WolframExample: Rust libraries from the Wolfram Language

This notebook exercises the WolframExample paclet, whose functions are implemented in Rust (see WolframExample/Libs). Evaluate it top to bottom.

If the setup section reports a missing paclet, build the library first:

```
cd WolframExample && cargo wl build
```

## Setup

Both paclets — the hand-written WolframExample and the generated WolframExampleLib — sit in this notebook's parent directory, so one PacletDirectoryLoad registers both.

```wolfram
PacletDirectoryLoad[ParentDirectory[NotebookDirectory[]]]
```

```wolfram
PacletObject["WolframExample"]
```

The library paclet holds nothing but the compiled dylibs and a generated Functions.wl loader:

```wolfram
PacletObject["WolframExampleLib"]
```

```wolfram
Needs["WolframExample`"]
```

Everything the package exports:

```wolfram
?WolframExample`*
```

## Math

### Scalars and arrays

A plain Rust function over two f64s. Arguments and result cross as raw MArgument scalars — no serialization at all.

```wolfram
ExampleAdd[2, 3]
```

NumericArrays are read zero-copy: Rust sees the kernel's own memory, no copy and no conversion. Lists are packed into a Real64 NumericArray by the wrapper first.

```wolfram
ExampleDot[{1, 2, 3}, {4, 5, 6}]
```

```wolfram
ExampleDot[
    NumericArray[Range[1., 100000.], "Real64"],
    NumericArray[Range[100000., 1., -1.], "Real64"]
]
```

The Rust side accumulates into 8 independent lanes so that LLVM can vectorize the loop (see Libs/math/src/lib.rs) — about twice the throughput of the naive left-to-right sum. Timed against the kernel's own Dot, which calls into BLAS; a Speedup above 1 means the Rust library won.

Two things decide what you see here. Time only the call, not the packing: both arrays are converted to NumericArrays first, since passing plain lists spends about three times the call itself on the conversion. And build with

```
cargo wl build --release
```

— cargo wl test leaves an unoptimized build in place, which loses by an order of magnitude.

```wolfram
With[
    {a = Range[1., 100000.], b = Range[100000., 1., -1.]},
    {na = NumericArray[a, "Real64"], nb = NumericArray[b, "Real64"]},
    {rust = First @ RepeatedTiming @ ExampleDot[na, nb],
     kernel = First @ RepeatedTiming @ Dot[a, b],
     packing = First @ RepeatedTiming @ ExampleDot[a, b]},
    <|
        "Rust" -> Quantity[rust, "Seconds"],
        "Kernel" -> Quantity[kernel, "Seconds"],
        "Speedup" -> kernel / rust,
        "RustFromLists" -> Quantity[packing, "Seconds"]
    |>
]
```

### Structs and enums

Structured arguments cross as WXF. A Rust struct is an Association keyed by its field names; the wrapper here presents Wolfram-style capitalized keys.

```wolfram
ExampleSymmetricPoint[<|"X" -> 10, "Y" -> 20|>]
```

ExampleArea goes through a Rust enum — one function, one match, two shapes:

```wolfram
ExampleArea[<|"Radius" -> 1|>]
```

```wolfram
ExampleArea[<|"Width" -> 10, "Height" -> 20|>]
```

```wolfram
Table[ExampleArea[<|"Radius" -> r|>], {r, 1, 5}]
```

### Errors

A Rust Result<f64, String> comes back as {"Ok", value} or {"Err", message}; the wrapper turns the error branch into a Failure.

```wolfram
ExampleSafeDivide[10, 4]
```

```wolfram
ExampleSafeDivide[1, 0]
```

So the usual Failure handling applies:

```wolfram
FailureQ /@ {ExampleSafeDivide[1, 2], ExampleSafeDivide[1, 0]}
```

## URL encoding

ExampleURLEncode and ExampleURLDecode are drop-in equivalents of the kernel's own URLEncode and URLDecode, built on percent-encoding — the RFC 3986 half of the Rust url crate. Same character set, same uppercase hex, same treatment of a bare + on the way back.

```wolfram
ExampleURLEncode["Hello, world!"]
```

```wolfram
ExampleURLDecode["Hello%2C%20world%21"]
```

### Lists are the real interface

The Rust functions take a list and return a list, and that is all they take: crossing the library boundary costs one WXF round trip, so encoding n strings should be one call rather than n. The single-string form above is a two-line wrapper in Kernel/Functions/URL.wl — wrap, call, unwrap.

```wolfram
ExampleURLEncode[{"a+b", "7 x + 5 y", "Kurt G\[ODoubleDot]del"}]
```

Decoding is deliberately asymmetric, exactly as the kernel's is: a literal + is a space, while %2B is a plus sign. Anything that is not a complete escape is left alone.

```wolfram
ExampleURLDecode[{"a+b", "a%2Bb", "a+b%2Bc", "100%", "%zz"}]
```

### Agreement with the kernel

Not a spot check: every character in the Basic Multilingual Plane, encoded both ways and compared. Lone surrogates are left out because they have no UTF-8 form at all — a Wolfram string can hold one, a Rust str cannot, and the library boundary rejects the payload before any of this code runs.

```wolfram
bmp = FromCharacterCode /@ Join[Range[0, 16^^D7FF], Range[16^^E000, 2^16 - 1]];
Length[bmp]
```

```wolfram
Counts[MapThread[SameQ, {ExampleURLEncode[bmp], URLEncode /@ bmp}]]
```

```wolfram
Counts[MapThread[SameQ, {ExampleURLDecode[URLEncode /@ bmp], bmp}]]
```

### Benchmark

Both directions, against the kernel's own implementation, over lists of 40-character strings drawn from an alphabet that is mostly punctuation and non-ASCII — so almost every character needs escaping. Speedup above 1 means the Rust library won.

At n = 1 the WXF round trip is most of the call and the margin is small; by n = 100 it has amortized away and the ratio settles. As with ExampleDot, build with

```
cargo wl build --release
```

— cargo wl test leaves an unoptimized build in place.

```wolfram
SeedRandom[42];
alphabet = Join[Characters["abcdefgh ?&=/:%"], {"\[ODoubleDot]", "\[Alpha]"}];
sample[n_] := Table[StringJoin[RandomChoice[alphabet, 40]], n];
```

```wolfram
row[n_] := Module[{data = sample[n], encoded},
    encoded = URLEncode /@ data;
    <|
        "n" -> n,
        "EncodeSpeedup" -> First[RepeatedTiming[URLEncode /@ data]] /
                           First[RepeatedTiming[ExampleURLEncode[data]]],
        "DecodeSpeedup" -> First[RepeatedTiming[URLDecode /@ encoded]] /
                           First[RepeatedTiming[ExampleURLDecode[encoded]]]
    |>
];
Dataset[row /@ {1, 10, 100, 1000, 10000}]
```

## DuckDB

An entire embedded SQL engine, statically linked into the library — nothing to install. Query results come back as Arrow and are decoded into a Tabular.

### Scoped connections

DuckDBExecute opens a connection, applies the function to it, and closes it again — even if the body fails.

```wolfram
DuckDBExecute["duckdb://", DuckDBQuery[#, "SELECT 42 AS answer"] &]
```

```wolfram
DuckDBExecute["duckdb://", DuckDBQuery[#,
    "SELECT n::INTEGER AS id,
            (n * 1.5)::DOUBLE AS score,
            'item_' || n AS label,
            n % 2 = 0 AS is_even
     FROM generate_series(1, 5) t(n)"] &]
```

The result is an ordinary Tabular, so the usual operations work:

```wolfram
tab = DuckDBExecute["duckdb://", DuckDBQuery[#,
    "SELECT n FROM generate_series(1, 100) t(n)"] &];
{Length[tab], Total[tab[[All, "n"]]]}
```

### Parameters

Values are bound to the ? placeholders in order, so nothing is interpolated into the SQL string.

```wolfram
DuckDBExecute["duckdb://", DuckDBQuery[#,
    "SELECT n FROM generate_series(1, 5) t(n) WHERE n > ?", {2}] &]
```

```wolfram
DuckDBExecute["duckdb://", DuckDBQuery[#,
    "SELECT ?::INTEGER + ?::INTEGER AS total", {20, 22}] &]
```

### Explicit connections

For several statements against the same database, hold the handle yourself. Here an on-disk file, which persists after disconnecting.

```wolfram
file = FileNameJoin[{$TemporaryDirectory, "wolfram-example.duckdb"}];
Quiet[DeleteFile[file]];
conn = DuckDBConnect["duckdb:///" <> file]
```

```wolfram
DuckDBQuery[conn,
    "CREATE TABLE fruit AS SELECT * FROM (VALUES (1, 'apple'), (2, 'pear')) v(id, name)"]
```

```wolfram
DuckDBQuery[conn, "INSERT INTO fruit VALUES (?, ?)", {3, "plum"}]
```

```wolfram
DuckDBQuery[conn, "SELECT * FROM fruit ORDER BY id"]
```

```wolfram
DuckDBDisconnect[conn]
```

Reopening the same file shows the rows are still there:

```wolfram
DuckDBExecute["duckdb:///" <> file,
    DuckDBQuery[#, "SELECT count(*) AS rows FROM fruit"] &]
```

### Failures

Errors from the engine arrive as Failure objects carrying DuckDB's own message, and propagate on their own.

```wolfram
DuckDBExecute["duckdb://", DuckDBQuery[#, "SELECT * FROM no_such_table"] &]
```

A handle that has been closed is no longer in the Rust-side registry:

```wolfram
With[{c = DuckDBConnect[]},
    DuckDBDisconnect[c];
    DuckDBQuery[c, "SELECT 1"]
]
```

## The raw library

The wrappers in Kernel/WolframExample.wl are a thin layer over an association of LibraryFunctions, keyed "namespace::rust_name". Anything without a wrapper is still one lookup away.

```wolfram
Keys[WolframExampleFunctions[]]
```

inactive_sum builds an expression in Rust and returns it unevaluated:

```wolfram
WolframExampleFunctions[]["math::inactive_sum"][{1, 2, 3}]
```

```wolfram
Activate[WolframExampleFunctions[]["math::inactive_sum"][{1, 2, 3}]]
```

A panic on the Rust side is caught at the boundary and returned as a Failure — it does not take the kernel down:

```wolfram
WolframExampleFunctions[]["math::panic"][]
```

And the signatures of everything that was packaged, straight from the generated Artifacts.wl asset:

```wolfram
Get[PacletObject["WolframExampleLib"]["AssetLocation", "Artifacts"]] //
    Map[Query[{"Name", "Signatures"}]] // Dataset
```

## Licensing

Everything the library links is compiled into the dylib, so the paclet has to carry its own attribution. cargo wl build writes that out as a third asset, License.wl: one entry per Rust package in the resolved dependency graph, dev-dependencies excluded.

```wolfram
licenses = Get[PacletObject["WolframExampleLib"]["AssetLocation", "License"]];
Length[licenses]
```

Each entry carries what the crate's own Cargo.toml declared — undeclared fields come back as Missing["NotAvailable"]:

```wolfram
Dataset[Take[licenses, 8]]
```

Which licenses the paclet actually ships under, most common first. The values are SPDX expressions, so an OR means the crate lets you pick:

```wolfram
ReverseSort[Counts[licenses[[All, "License"]]]] // Dataset
```

The entries worth a human's attention — anything that declared no license at all. Here that is the two example crates themselves: neither Libs/math nor Libs/duckdb sets a license field, which a paclet you actually shipped would want to fix:

```wolfram
Select[licenses, MissingQ[#["License"]] &] // Dataset
```

And the direct dependencies of the two example crates, as opposed to the whole transitive graph:

```wolfram
Select[licenses,
    MemberQ[{"duckdb", "arrow-ipc", "uuid", "wolfram-expr",
             "wolfram-export", "wolfram-serialize", "wolfram-library-link"},
        #["Name"]] &] // Dataset
```

## Rebuilding

After editing the Rust source, rebuild from the paclet directory. The dylibs are content-hashed, so a rebuilt library gets a new file name — quit the kernel to pick it up.

```wolfram
ExternalEvaluate[{"Shell",
    "ProcessDirectory" -> FileNameJoin[{ParentDirectory[NotebookDirectory[]], "WolframExample"}],
    "ProcessEnvironment" -> <|"PATH" -> Environment["PATH"]|>},
   "cargo wl build --release"]
```
