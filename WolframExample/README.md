# WolframExample

A minimal Wolfram Language paclet whose functionality comes from Rust. It is
the smallest complete example of the split this repo's tooling is built around:

* **`WolframExample`** (this directory) — a plain, hand-written paclet: a
  `PacletInfo.wl` with a Kernel extension and one `.wl` file of top-level code.
  No binaries, no generated files, nothing platform-specific.
* **`WolframExampleLib`** — a *generated* paclet containing only compiled
  dylibs and a `Functions.wl` loader. It is produced by `cargo wl build` from
  the crates in [`Libs/`](Libs), one directory per platform
  (`WolframExampleLib-MacOSX-ARM64`, `WolframExampleLib-Linux-x86-64`, …).

Keeping them apart means the code you edit and the code you build never mix:
`WolframExampleLib-*` can be deleted and regenerated at will, and
`WolframExample` is what you would put in a repository, ship, or install.

```
WolframExample/
├── PacletInfo.wl               the paclet (Kernel extension, context WolframExample`)
├── Cargo.toml                  standalone Cargo workspace — run cargo wl from here
├── Kernel/
│   ├── WolframExample.wl       declares the public symbols, then loads the rest
│   ├── Code/                   shared helpers (the library loader)
│   └── Functions/              one file per Rust library, wrapping its functions
└── Libs/                       the Rust side
    ├── math/                   namespace "math":    scalars, NumericArrays, structs, enums
    ├── url/                    namespace "url":     strings in, strings out, in batches
    ├── duckdb/                 namespace "duckdb":  an embedded SQL engine
    └── tests/*.wlt             Wolfram-level tests

WolframExampleLib-MacOSX-ARM64/ generated next to this directory by cargo wl build
```

## How the two halves meet

Each crate in `Libs/` declares its packaging in its own `Cargo.toml`:

```toml
[package.metadata.wl.pacletinfo]
name = "WolframExampleLib"   # both crates package into one paclet
version = "1.0.0"
namespace = "math"           # per-crate key prefix: "math::add"
output = "../../../"         # written next to the WolframExample paclet
```

`cargo wl build` compiles every crate to a `cdylib`, reads the function
manifest each one embeds, and writes a single
`WolframExampleLib-<SystemID>/` directory containing the dylibs plus:

* `Functions.wl` — evaluates to `<|"math::add" -> LibraryFunction[…], …|>`,
  every exported Rust function already loaded and ready to call;
* `Artifacts.wl` — one descriptor per dylib (hash, path, signatures);
* `License.wl` — the license, version and authors of every Rust crate linked
  in, dependencies included, so the shipped paclet carries its attribution;
* `PacletInfo.wl` — declares those files as the paclet's `"Functions"`,
  `"Artifacts"` and `"License"` assets.

`Kernel/WolframExample.wl` never looks at a path or a platform directory. It
asks the paclet manager for the asset:

```wolfram
paclet = PacletObject["WolframExampleLib"];
functions = Get[paclet["AssetLocation", "Functions"]];
functions["duckdb::db_query"][conn, "SELECT 42", <||>]
```

Note the property is **`"AssetLocation"`** (singular), with the asset name as
the second argument. Everything else in the kernel file is boilerplate around
that association: argument patterns, WL-shaped names, and conversion of the
Rust-side data shapes into idiomatic Wolfram results.

## Building

From `WolframExample`:

```shell
cargo install cargo-wl     # once
cargo wl build --release   # writes ../WolframExampleLib-<SystemID>/
```

The DuckDB crate compiles the engine from source, so the first build takes a
while; later builds are incremental.

## Using it

Both paclets live in the same parent directory, so one `PacletDirectoryLoad`
of that directory registers both:

```wolfram
PacletDirectoryLoad["/path/to/wolfram-rust-library"];
Needs["WolframExample`"]

ExampleAdd[2, 3]                                    (* 5. *)
ExampleDot[{1, 2, 3}, {4, 5, 6}]                    (* 32. *)
ExampleArea[<|"Radius" -> 1|>]                      (* 3.14159 *)
ExampleArea[<|"Width" -> 10, "Height" -> 20|>]      (* 200. *)
ExampleSymmetricPoint[<|"X" -> 10, "Y" -> 20|>]     (* <|"X" -> -10., "Y" -> -20.|> *)
ExampleSafeDivide[1, 0]                             (* Failure["DivisionError", …] *)
```

URL encoding, matching `System`URLEncode` / `URLDecode` exactly:

```wolfram
ExampleURLEncode["Hello, world!"]                   (* "Hello%2C%20world%21" *)
ExampleURLDecode["a+b"]                             (* "a b" — a bare + is a space *)
ExampleURLDecode["a%2Bb"]                           (* "a+b" — but %2B is a plus *)
```

The Rust side takes and returns a **list**, which is the point: one call across
the library boundary costs a WXF round trip either way, so encoding 10,000
strings should be one call, not 10,000. The single-string forms above are
one-line wrappers in `Kernel/Functions/URL.wl`.

```wolfram
ExampleURLEncode[{"a+b", "7 x + 5 y", "Kurt Gödel"}]
```

Batched, this runs several times faster than mapping the kernel's own
`URLEncode` over the same list; per string, the cost stops falling at about 100
strings per call. Below roughly ten it is a wash, and for a *single* string the
boundary crossing costs more than `URLEncode` does — use the kernel's
built-in there.

DuckDB, with the connection handled for you:

```wolfram
DuckDBExecute["duckdb://", DuckDBQuery[#, "SELECT 42 AS x"] &]

DuckDBExecute["duckdb://", DuckDBQuery[#,
    "SELECT n FROM generate_series(1, 5) t(n) WHERE n > ?", {2}] &]
```

or manually:

```wolfram
conn = DuckDBConnect["duckdb:///tmp/example.db"];
DuckDBQuery[conn, "CREATE TABLE t AS SELECT 1 AS a"];
DuckDBQuery[conn, "SELECT * FROM t"];
DuckDBDisconnect[conn];
```

`DuckDBConnect` also accepts `postgres://`, `sqlite://` and `mysql://` URLs,
which are ATTACHed into an in-memory DuckDB instance. Query results come back
as a `Tabular`; failures come back as `Failure["ExecuteError", <|…|>]` and so
propagate on their own.

To reach a function that has no wrapper yet, use the raw association:

```wolfram
WolframExampleFunctions[]["math::inactive_sum"][{1, 2, 3}]
```

## The tour notebook

`notebooks/WolframExampleTour.nb` runs everything above, section by section,
with commentary. It is generated — edit the cells in
`scripts/generate-example-notebook.wls` and regenerate:

```shell
wolframscript -file scripts/generate-example-notebook.wls
wolframscript -file scripts/generate-example-notebook.wls --check   # evaluate every cell
```

`--check` evaluates each input cell in a kernel and reports any that fail, so a
tour cell that goes stale after an API change is caught without opening the
front end.

## Adding a function

1. Write it in `Libs/math/src/lib.rs` (or a new crate) and tag it:
   `#[export]` for plain scalar/NumericArray arguments, `#[export(wxf)]` when
   arguments or results are structs, enums, or `Expr`s.
2. Rebuild: `cargo wl build`. The new key appears in `Functions.wl` as
   `"<namespace>::<rust_fn_name>"`.
3. Add a wrapper in `Kernel/WolframExample.wl` — a `::usage`, an argument
   pattern, and whatever reshaping makes it read like Wolfram code.

Data crosses as WXF, in these shapes:

| Rust                     | Wolfram                                            |
| ------------------------ | -------------------------------------------------- |
| `struct Point { x, y }`  | `<\|"x" -> 1., "y" -> 2.\|>` (or positional `{1., 2.}`) |
| `enum Shape { Circle(…) }` | `{"Circle", <\|"radius" -> 1.\|>}`                |
| `Result<f64, String>`    | `{"Ok", 1.5}` / `{"Err", "message"}`               |
| `#[derive(Failure)]` enum | `Failure["VariantName", <\|…\|>]`                 |
| a panic                  | `Failure["RustPanic", <\|…\|>]`                    |

Struct fields keep their Rust names unless a `key_processor` or `rename` is
set — see `wolfram-serialize`'s docs.

## Testing

From `WolframExample`:

```shell
cargo wl test
```

This builds the crates, packages them, then runs every `.wlt` under the
current directory in a Wolfram kernel:

* `Libs/tests/Duckdb.wlt` tests the library directly, calling the raw
  `LibraryFunction`s out of `Functions.wl`.
* `Libs/tests/WolframExample.wlt` tests the whole chain — it `PacletDirectoryLoad`s
  the parent directory (where both paclets sit), `Needs` the
  `WolframExample`` ` context, and calls the top-level API.

## Note on dependencies

The crates in `Libs/` depend on the **published crates.io releases** of this
repo's crates, not on local paths, and form their own Cargo workspace excluded
from the root one. That is deliberate: the directory is meant to be copied out
and used as a starting point for a real paclet, unchanged.
