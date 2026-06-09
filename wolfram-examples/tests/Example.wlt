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
               Messages -> expected messages ({} for none, {_} for any one),
               TestID -> unique string identifier. *)

$Tests = {

    (* ── legacy_wstp: echo atoms ─────────────────────────────────────────────── *)

    <|"Export" -> "legacy_wstp::echo_expr",
      "Input"  -> {42},
      "Output" -> 42,
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-echo_expr-integer"|>,

    <|"Export" -> "legacy_wstp::echo_expr",
      "Input"  -> {-7},
      "Output" -> -7,
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-echo_expr-negative_integer"|>,

    <|"Export" -> "legacy_wstp::echo_expr",
      "Input"  -> {1.5},
      "Output" -> 1.5,
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-echo_expr-real"|>,

    (* ── legacy_wstp: big numbers ────────────────────────────────────────────── *)

    <|"Export" -> "legacy_wstp::echo_expr",
      "Input"  -> {2^200},
      "Output" -> 2^200,
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-echo_expr-big_integer"|>,

    <|"Export" -> "legacy_wstp::echo_expr",
      "Input"  -> {N[Pi, 50]},
      "Output" -> N[Pi, 50],
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-echo_expr-big_real"|>,

    (* ── legacy_wstp: packed types ───────────────────────────────────────────── *)

    <|"Export" -> "legacy_wstp::make_byte_array",
      "Input"  -> {},
      "Output" -> ByteArray[{1, 2, 3}],
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-make_byte_array"|>,

    <|"Export" -> "legacy_wstp::make_numeric_array_r64",
      "Input"  -> {},
      "Output" -> NumericArray[{1., 2., 3.}, "Real64"],
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-make_numeric_array_r64"|>,

    <|"Export" -> "legacy_wstp::make_numeric_array_i32_2d",
      "Input"  -> {},
      "Output" -> NumericArray[{{1, 2}, {3, 4}}, "Integer32"],
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-make_numeric_array_i32_2d"|>,

    (* ── legacy_wstp: kind tags ──────────────────────────────────────────────── *)

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {42},
      "Output" -> "Integer",
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-integer"|>,

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {1.5},
      "Output" -> "Real",
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-real"|>,

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {2^200},
      "Output" -> "BigInteger",
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-big_integer"|>,

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {N[Pi, 50]},
      "Output" -> "BigReal",
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-big_real"|>,

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {"hello"},
      "Output" -> "String",
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-string"|>,

    <|"Export" -> "legacy_wstp::expr_kind_tag",
      "Input"  -> {Pi},
      "Output" -> "Symbol",
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-symbol"|>,

    (* ── mixed: one function per export mode ─────────────────────────────────── *)

    <|"Export" -> "mixed::add",
      "Input"  -> {3.0, 4.0},
      "Output" -> 7.0,
      "Messages" -> {},
      "TestID" -> "Examples-mixed-add"|>,

    <|"Export" -> "mixed::reverse",
      "Input"  -> {{10, 20, 30}},
      "Output" -> {30, 20, 10},
      "Messages" -> {},
      "TestID" -> "Examples-mixed-reverse"|>,

    <|"Export" -> "mixed::dot",
      "Input"  -> {NumericArray[{1., 2., 3.}, "Real64"], NumericArray[{4., 5., 6.}, "Real64"]},
      "Output" -> 32.0,
      "Messages" -> {},
      "TestID" -> "Examples-mixed-dot"|>,

    (* ── types_wxf: echo_point ───────────────────────────────────────────────── *)

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {{1, 2}},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf-echo_point-list"|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {ByteArray[{1, 2}]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf-echo_point-byte_array"|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {NumericArray[{1, 2}]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf-echo_point-numeric_array_u8"|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {NumericArray[{1, 2}, "Integer32"]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf-echo_point-numeric_array_i32"|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {<|"x" -> 1, "y" -> 2|>},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf-echo_point-assoc_xy"|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {<|"y" -> 2, "x" -> 1|>},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf-echo_point-assoc_yx"|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {Hold[1, 2]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf-echo_point-hold"|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {{1}},
      "Output" -> _Failure,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf-echo_point-wrong_length"|>,

    <|"Export" -> "types_wxf::echo_point",
      "Input"  -> {"hello"},
      "Output" -> _Failure,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf-echo_point-wrong_type"|>,

    (* ── panic tests ─────────────────────────────────────────────────────────── *)

    <|"Export" -> "types_native::force_panic",
      "Input"  -> {42.0},
      "Output" -> _LibraryFunctionError,
      "Messages" -> {_},
      "TestID" -> "Examples-types_native-force_panic"|>,

    <|"Export" -> "types_wstp::force_panic",
      "Input"  -> {42.0},
      "Output" -> _Failure,
      "Messages" -> {},
      "TestID" -> "Examples-types_wstp-force_panic"|>,

    <|"Export" -> "types_wxf::force_panic",
      "Input"  -> {42.0},
      "Output" -> _Failure,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf-force_panic"|>,

    (* ── types_wxf_ffi: scalars ───────────────────────────────────────────────── *)

    <|"Export" -> "types_wxf_ffi::add",
      "Input"  -> {3.0, 4.0},
      "Output" -> 7.0,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf_ffi-add"|>,

    <|"Export" -> "types_wxf_ffi::dot",
      "Input"  -> {NumericArray[{1., 2., 3.}, "Real64"], NumericArray[{4., 5., 6.}, "Real64"]},
      "Output" -> 32.0,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf_ffi-dot"|>,

    <|"Export" -> "types_wxf_ffi::scale_array",
      "Input"  -> {NumericArray[{1., 2., 3.}, "Real64"], 2.0},
      "Output" -> NumericArray[{2., 4., 6.}, "Real64"],
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf_ffi-scale_array"|>,

    (* ── types_wxf_ffi: Expr passthrough ─────────────────────────────────────── *)

    <|"Export" -> "types_wxf_ffi::duplicate",
      "Input"  -> {Hold[1, 2, 3]},
      "Output" -> Hold[1, 2, 3],
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf_ffi-duplicate"|>,

    (* ── types_wxf_ffi: typed struct round-trip ──────────────────────────────── *)

    <|"Export" -> "types_wxf_ffi::echo_point",
      "Input"  -> {<|"x" -> 1.0, "y" -> 2.0|>},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf_ffi-echo_point"|>,

    <|"Export" -> "types_wxf_ffi::summarize",
      "Input"  -> {<|"name" -> "test", "blob" -> ByteArray[{1, 2, 3}], "values" -> NumericArray[{1., 2., 3.}, "Real64"]|>},
      "Output" -> _String,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf_ffi-summarize"|>,

    (* ── types_wxf_ffi: panic surfaces as Failure ────────────────────────────── *)

    <|"Export" -> "types_wxf_ffi::force_panic",
      "Input"  -> {42.0},
      "Output" -> _Failure,
      "Messages" -> {},
      "TestID" -> "Examples-types_wxf_ffi-force_panic"|>

};

(* ── Runner ──────────────────────────────────────────────────────────────────── *)

TestCreate[
    Apply[$Libs[#Export], #Input],
    #Output,
    #Messages,
    SameTest -> MatchQ,
    TestID -> #TestID
] & /@ $Tests
