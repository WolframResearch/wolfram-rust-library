(* Build all examples from the repo root:
     cargo wl build --example legacy_wstp --example mixed \
       --example types_native --example types_wstp --example types_wxf \
       -p wolfram-examples
   Then run this file:
     wolframscript -code 'TestReport["wolfram-examples/tests/Example.wl"]' *)

(* ── Load libraries ─────────────────────────────────────────────────────────── *)

$thisDir = DirectoryName[If[$TestFileName =!= "", $TestFileName, $InputFileName]];

loadLib[name_String] := Module[{path, lib},
    path = FileNameJoin[{$thisDir, "..", "..", "target", "debug", "examples",
                         name, $SystemID, name, "manifest.wl"}];
    lib = Quiet[Get[path]];
    If[AssociationQ[lib], lib,
        Print["SKIP: could not load ", name, " from ", path]; Missing["NotLoaded"]]
];

$LibraryNames = {"liblegacy_wstp", "libmixed", "libtypes_native", "libtypes_wstp", "libtypes_wxf"};

$Libs = AssociationMap[loadLib, $LibraryNames];

Print["loaded: ", Select[$Libs, AssociationQ] // Keys // StringRiffle[#, ", "] &];

(* ── Test definitions ────────────────────────────────────────────────────────── *)
(* Each entry: Library -> lib name, Function -> function key,
               Input -> list of arguments, Output -> expected return value,
               Messages -> expected messages ({} for none, {_} for any one). *)

$Tests = {

    (* ── legacy_wstp: echo atoms ─────────────────────────────────────────────── *)

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "echo_expr",
      "Input"     -> {42},
      "Output"    -> 42,
      "Messages"  -> {}|>,

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "echo_expr",
      "Input"     -> {-7},
      "Output"    -> -7,
      "Messages"  -> {}|>,

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "echo_expr",
      "Input"     -> {1.5},
      "Output"    -> 1.5,
      "Messages"  -> {}|>,

    (* ── legacy_wstp: big numbers ────────────────────────────────────────────── *)

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "echo_expr",
      "Input"     -> {2^200},
      "Output"    -> 2^200,
      "Messages"  -> {}|>,

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "echo_expr",
      "Input"     -> {N[Pi, 50]},
      "Output"    -> N[Pi, 50],
      "Messages"  -> {}|>,

    (* ── legacy_wstp: packed types ───────────────────────────────────────────── *)

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "make_byte_array",
      "Input"     -> {},
      "Output"    -> ByteArray[{1, 2, 3}],
      "Messages"  -> {}|>,

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "make_numeric_array_r64",
      "Input"     -> {},
      "Output"    -> NumericArray[{1., 2., 3.}, "Real64"],
      "Messages"  -> {}|>,

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "make_numeric_array_i32_2d",
      "Input"     -> {},
      "Output"    -> NumericArray[{{1, 2}, {3, 4}}, "Integer32"],
      "Messages"  -> {}|>,

    (* ── legacy_wstp: kind tags ──────────────────────────────────────────────── *)

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "expr_kind_tag",
      "Input"     -> {42},
      "Output"    -> "Integer",
      "Messages"  -> {}|>,

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "expr_kind_tag",
      "Input"     -> {1.5},
      "Output"    -> "Real",
      "Messages"  -> {}|>,

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "expr_kind_tag",
      "Input"     -> {2^200},
      "Output"    -> "BigInteger",
      "Messages"  -> {}|>,

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "expr_kind_tag",
      "Input"     -> {N[Pi, 50]},
      "Output"    -> "BigReal",
      "Messages"  -> {}|>,

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "expr_kind_tag",
      "Input"     -> {"hello"},
      "Output"    -> "String",
      "Messages"  -> {}|>,

    <|"Library"   -> "liblegacy_wstp",
      "Function"  -> "expr_kind_tag",
      "Input"     -> {Pi},
      "Output"    -> "Symbol",
      "Messages"  -> {}|>,

    (* ── mixed: one function per export mode ─────────────────────────────────── *)

    <|"Library"   -> "libmixed",
      "Function"  -> "add",
      "Input"     -> {3.0, 4.0},
      "Output"    -> 7.0,
      "Messages"  -> {}|>,

    <|"Library"   -> "libmixed",
      "Function"  -> "reverse",
      "Input"     -> {{10, 20, 30}},
      "Output"    -> {30, 20, 10},
      "Messages"  -> {}|>,

    <|"Library"   -> "libmixed",
      "Function"  -> "dot",
      "Input"     -> {NumericArray[{1., 2., 3.}, "Real64"], NumericArray[{4., 5., 6.}, "Real64"]},
      "Output"    -> 32.0,
      "Messages"  -> {}|>,

    (* ── types_wxf: echo_point accepts multiple 2-element numeric wire shapes ── *)

    <|"Library"   -> "libtypes_wxf",
      "Function"  -> "echo_point",
      "Input"     -> {{1, 2}},
      "Output"    -> <|"x" -> 1., "y" -> 2.|>,
      "Messages"  -> {}|>,

    <|"Library"   -> "libtypes_wxf",
      "Function"  -> "echo_point",
      "Input"     -> {ByteArray[{1, 2}]},
      "Output"    -> <|"x" -> 1., "y" -> 2.|>,
      "Messages"  -> {}|>,

    <|"Library"   -> "libtypes_wxf",
      "Function"  -> "echo_point",
      "Input"     -> {NumericArray[{1, 2}]},
      "Output"    -> <|"x" -> 1., "y" -> 2.|>,
      "Messages"  -> {}|>,

    <|"Library"   -> "libtypes_wxf",
      "Function"  -> "echo_point",
      "Input"     -> {NumericArray[{1, 2}, "Integer32"]},
      "Output"    -> <|"x" -> 1., "y" -> 2.|>,
      "Messages"  -> {}|>,

    (* Association with keys in declaration order *)
    <|"Library"   -> "libtypes_wxf",
      "Function"  -> "echo_point",
      "Input"     -> {<|"x" -> 1, "y" -> 2|>},
      "Output"    -> <|"x" -> 1., "y" -> 2.|>,
      "Messages"  -> {}|>,

    (* Association with keys in reverse order — still deserializes correctly *)
    <|"Library"   -> "libtypes_wxf",
      "Function"  -> "echo_point",
      "Input"     -> {<|"y" -> 2, "x" -> 1|>},
      "Output"    -> <|"x" -> 1., "y" -> 2.|>,
      "Messages"  -> {}|>,

    (* Function head is discarded — Hold[1, 2] has 2 args so positional branch succeeds *)
    <|"Library"   -> "libtypes_wxf",
      "Function"  -> "echo_point",
      "Input"     -> {Hold[1, 2]},
      "Output"    -> <|"x" -> 1., "y" -> 2.|>,
      "Messages"  -> {}|>,

    (* Wrong element count — {1} has 1 arg, Point needs 2 *)
    <|"Library"   -> "libtypes_wxf",
      "Function"  -> "echo_point",
      "Input"     -> {{1}},
      "Output"    -> _Failure,
      "Messages"  -> {}|>,

    (* Wrong type — a String can't be decoded as Point *)
    <|"Library"   -> "libtypes_wxf",
      "Function"  -> "echo_point",
      "Input"     -> {"hello"},
      "Output"    -> _Failure,
      "Messages"  -> {}|>,

    (* ── panic tests ─────────────────────────────────────────────────────────── *)
    (* wstp/wxf return Failure; native returns LibraryFunctionError + one message *)

    <|"Library"   -> "libtypes_native",
      "Function"  -> "force_panic",
      "Input"     -> {42.0},
      "Output"    -> _LibraryFunctionError,
      "Messages"  -> {_}|>,

    <|"Library"   -> "libtypes_wstp",
      "Function"  -> "force_panic",
      "Input"     -> {42.0},
      "Output"    -> _Failure,
      "Messages"  -> {}|>,

    <|"Library"   -> "libtypes_wxf",
      "Function"  -> "force_panic",
      "Input"     -> {42.0},
      "Output"    -> _Failure,
      "Messages"  -> {}|>

};

(* ── Runner ──────────────────────────────────────────────────────────────────── *)

TestCreate[
    Apply[$Libs[#Library][#Function], #Input],
    #Output,
    #Messages,
    SameTest -> MatchQ
] & /@ $Tests
