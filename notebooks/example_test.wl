(* Test script for legacy_native and legacy_wstp examples.
   Run with: wolframscript -file notebooks/example_test.wl
   from the wolfram-rust-library workspace root. *)

$root = ParentDirectory[DirectoryName[$InputFileName]];
$nativeLib = FileNameJoin[{$root, "target/debug/examples/liblegacy_native.dylib"}];
$wstpLib   = FileNameJoin[{$root, "target/debug/examples/liblegacy_wstp.dylib"}];

(* ── Helpers ─────────────────────────────────────────────────────────────── *)

$passed = 0;
$failed = 0;

check[label_String, got_, expected_] := If[got === expected,
    $passed++;
    Print["  PASS  ", label],
    $failed++;
    Print["  FAIL  ", label, "  got: ", got, "  expected: ", expected]
];

(* ── Load via loaders ────────────────────────────────────────────────────── *)

Print["\n=== Loading native functions ==="];
loadNative = LibraryFunctionLoad[$nativeLib, "load_legacy_native_functions", LinkObject, LinkObject];
$n = loadNative[$nativeLib];
Print["Loaded: ", Keys[$n]];

Print["\n=== Loading WSTP functions ==="];
loadWstp = LibraryFunctionLoad[$wstpLib, "load_legacy_wstp_functions", LinkObject, LinkObject];
$w = loadWstp[$wstpLib];
Print["Loaded: ", Keys[$w]];

(* ══════════════════════════════════════════════════════════════════════════ *)
Print["\n=== Native tests ==="];

check["square[7]",          $n["square"][7],             49];
check["square[-3]",         $n["square"][-3],            9];
check["add[3.0, 4.0]",      $n["add"][3.0, 4.0],        7.0];
check["add[0.5, 0.5]",      $n["add"][0.5, 0.5],        1.0];
check["reverse_string",     $n["reverse_string"]["hello"], "olleh"];
check["reverse_string empty",$n["reverse_string"][""],   ""];

check["total_i64",
    $n["total_i64"][NumericArray[{1, 2, 3, 4}, "Integer64"]],
    10];

check["dot_f64",
    $n["dot_f64"][
        NumericArray[{1.0, 2.0, 3.0}, "Real64"],
        NumericArray[{4.0, 5.0, 6.0}, "Real64"]],
    32.0];  (* 1*4 + 2*5 + 3*6 *)

check["scale_f64",
    Normal @ $n["scale_f64"][NumericArray[{1.0, 2.0, 3.0}, "Real64"], 2.0],
    {2.0, 4.0, 6.0}];

check["positive_i64",
    Normal @ $n["positive_i64"][NumericArray[{-2, 0, 3, -1, 5}, "Integer64"]],
    {0, 0, 1, 0, 1}];

(* ══════════════════════════════════════════════════════════════════════════ *)
Print["\n=== WSTP tests ==="];

check["square_wstp[5]",     $w["square_wstp"][5],        25];
check["square_wstp[-4]",    $w["square_wstp"][-4],       16];
check["count_args 0",       $w["count_args"][],          0];
check["count_args 3",       $w["count_args"][a, b, c],   3];
check["total_args_i64",     $w["total_args_i64"][1, 2, 3, 4, 5], 15];
check["string_join",        $w["string_join"]["foo", "bar", "baz"], "foobarbaz"];
check["string_join empty",  $w["string_join"][],         ""];
check["link_expr_identity", $w["link_expr_identity"][1, 2, 3], {1, 2, 3}];
check["total integers",     $w["total"][1, 2, 3, 4],    10];
check["total reals",        $w["total"][1.0, 2.0, 3.0], 6.0];
check["expr_string_join",   $w["expr_string_join"]["x", "y", "z"], "xyz"];

(* ── Summary ─────────────────────────────────────────────────────────────── *)

Print["\n=== Summary: ", $passed, " passed, ", $failed, " failed ===\n"];
If[$failed > 0, Exit[1]];
