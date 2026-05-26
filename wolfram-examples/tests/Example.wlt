(* Run via: cargo wl test (from wolfram-examples/) *)

(* ── Load libraries ─────────────────────────────────────────────────────────── *)
(* $LibraryPath and SetDirectory are already set by cargo wl test *)

$Libs = Quiet[Get["Functions.wl"]];

If[AssociationQ[$Libs],
    Print["loaded ", Length[$Libs], " functions"],
    Print["SKIP: could not load Functions.wl"]; $Libs = <||>
];

(* ── Test definitions ────────────────────────────────────────────────────────── *)
(* Each entry: Export -> "libname::fnname",
               Input -> list of arguments, Output -> expected return value,
               Messages -> expected messages ({} for none, {_} for any one). *)

$Tests = {

    (* ── legacy_wstp: echo atoms ─────────────────────────────────────────────── *)

    <|"Export" -> "legacy_wstp::echo_expr",
      "Input"  -> {42},
      "Output" -> 42,
      "Messages" -> {}|>,

    <|"Export" -> "legacy_wstp::echo_expr",
      "Input"  -> {-7},
      "Output" -> -7,
      "Messages" -> {}|>,

    <|"Export" -> "legacy_wstp::echo_expr",
      "Input"  -> {1.5},
      "Output" -> 1.5,
      "Messages" -> {}|>,

    (* ── legacy_wstp: big numbers ────────────────────────────────────────────── *)

    <|"Export" -> "legacy_wstp::echo_expr",
      "Input"  -> {2^200},
      "Output" -> 2^200,
      "Messages" -> {}|>,

    <|"Export" -> "legacy_wstp::echo_expr",
      "Input"  -> {N[Pi, 50]},
      "Output" -> N[Pi, 50],
      "Messages" -> {}|>,

    (* ── legacy_wstp: packed types ───────────────────────────────────────────── *)

    <|"Export" -> "legacy_wstp::make_byte_array",
      "Input"  -> {},
      "Output" -> ByteArray[{1, 2, 3}],
      "Messages" -> {}|>,

    <|"Export" -> "legacy_wstp::make_numeric_array_r64",
      "Input"  -> {},
      "Output" -> NumericArray[{1., 2., 3.}, "Real64"],
      "Messages" -> {}|>,

    <|"Export" -> "legacy_wstp::make_numeric_array_i32_2d",
      "Input"  -> {},
      "Output" -> NumericArray[{{1, 2}, {3, 4}}, "Integer32"],
      "Messages" -> {}|>,

    (* ── legacy_wstp: kind tags ──────────────────────────────────────────────── *)

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {42},
      "Output" -> "Integer",
      "Messages" -> {}|>,

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {1.5},
      "Output" -> "Real",
      "Messages" -> {}|>,

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {2^200},
      "Output" -> "BigInteger",
      "Messages" -> {}|>,

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {N[Pi, 50]},
      "Output" -> "BigReal",
      "Messages" -> {}|>,

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {"hello"},
      "Output" -> "String",
      "Messages" -> {}|>,

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {Pi},
      "Output" -> "Symbol",
      "Messages" -> {}|>,

    (* ── mixed: one function per export mode ─────────────────────────────────── *)

    <|"Export" -> "mixed::add",
      "Input"  -> {3.0, 4.0},
      "Output" -> 7.0,
      "Messages" -> {}|>,

    <|"Export" -> "mixed::reverse",
      "Input"  -> {{10, 20, 30}},
      "Output" -> {30, 20, 10},
      "Messages" -> {}|>,

    <|"Export" -> "mixed::dot",
      "Input"  -> {NumericArray[{1., 2., 3.}, "Real64"], NumericArray[{4., 5., 6.}, "Real64"]},
      "Output" -> 32.0,
      "Messages" -> {}|>,

    (* ── types_wxf: echo_point ───────────────────────────────────────────────── *)

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {{1, 2}},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {}|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {ByteArray[{1, 2}]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {}|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {NumericArray[{1, 2}]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {}|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {NumericArray[{1, 2}, "Integer32"]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {}|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {<|"x" -> 1, "y" -> 2|>},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {}|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {<|"y" -> 2, "x" -> 1|>},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {}|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {Hold[1, 2]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {}|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {{1}},
      "Output" -> _Failure,
      "Messages" -> {}|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {"hello"},
      "Output" -> _Failure,
      "Messages" -> {}|>,

    (* ── panic tests ─────────────────────────────────────────────────────────── *)

    <|"Export" -> "types_native::force_panic",
      "Input"  -> {42.0},
      "Output" -> _LibraryFunctionError,
      "Messages" -> {_}|>,

    <|"Export" -> "types_wstp::force_panic",
      "Input"  -> {42.0},
      "Output" -> _Failure,
      "Messages" -> {}|>,

    <|"Export" -> "types_wxf::force_panic",
      "Input"  -> {42.0},
      "Output" -> _Failure,
      "Messages" -> {}|>

};

(* ── Runner ──────────────────────────────────────────────────────────────────── *)

TestCreate[
    Apply[$Libs[#Export], #Input],
    #Output,
    #Messages,
    SameTest -> MatchQ
] & /@ $Tests
