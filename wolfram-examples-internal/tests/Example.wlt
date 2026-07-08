(* Run via: cargo wl test (from wolfram-examples-internal/) *)

(* ── Load libraries ─────────────────────────────────────────────────────────── *)
(* $LibraryPath and SetDirectory are already set by cargo wl test *)

$Libs = Quiet[Get["Functions.wl"]];

If[AssociationQ[$Libs],
    Print["loaded ", Length[$Libs], " functions"],
    Print["SKIP: could not load Functions.wl"]; $Libs = <||>
];

(* ── Test definitions ────────────────────────────────────────────────────────── *)
(* This crate builds with namespace-exports off (it's a single crate, one
   module per export mode), so functions are keyed by their bare name; modules
   whose functions could collide (native/wstp/wxf mirror the same test battery)
   are disambiguated by a `native_`/`wstp_`/`wxf_` prefix in the Rust source
   instead. Each entry: Export -> "fnname",
               Input -> list of arguments, Output -> expected return value,
               Messages -> expected messages ({} for none, {_} for any one),
               TestID -> unique string identifier. *)

$Tests = {

    (* ── legacy_wstp: echo atoms ─────────────────────────────────────────────── *)

    <|"Export" -> "echo_expr",
      "Input"  -> {42},
      "Output" -> 42,
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-echo_expr-integer"|>,

    <|"Export" -> "echo_expr",
      "Input"  -> {-7},
      "Output" -> -7,
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-echo_expr-negative_integer"|>,

    <|"Export" -> "echo_expr",
      "Input"  -> {1.5},
      "Output" -> 1.5,
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-echo_expr-real"|>,

    (* ── legacy_wstp: big numbers ────────────────────────────────────────────── *)

    (* wstp's get_expr matches upstream master: WSGetInteger64/WSGetReal64 don't
       preserve arbitrary precision. A BigInteger errors out (overflow); a
       BigReal is silently truncated to a machine double. *)
    <|"Export" -> "echo_expr",
      "Input"  -> {2^200},
      "Output" -> _LibraryFunctionError,
      "Messages" -> {_},
      "TestID" -> "Examples-legacy_wstp-echo_expr-big_integer"|>,

    <|"Export" -> "echo_expr",
      "Input"  -> {N[Pi, 50]},
      "Output" -> N[Pi],
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-echo_expr-big_real"|>,

    (* ── legacy_wstp: packed types ───────────────────────────────────────────── *)

    <|"Export" -> "make_byte_array",
      "Input"  -> {},
      "Output" -> ByteArray[{1, 2, 3}],
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-make_byte_array"|>,

    <|"Export" -> "make_numeric_array_r64",
      "Input"  -> {},
      "Output" -> NumericArray[{1., 2., 3.}, "Real64"],
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-make_numeric_array_r64"|>,

    <|"Export" -> "make_numeric_array_i32_2d",
      "Input"  -> {},
      "Output" -> NumericArray[{{1, 2}, {3, 4}}, "Integer32"],
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-make_numeric_array_i32_2d"|>,

    (* ── legacy_wstp: kind tags ──────────────────────────────────────────────── *)

    <|"Export" -> "expr_kind_tag",
      "Input"  -> {42},
      "Output" -> "Integer",
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-integer"|>,

    <|"Export" -> "expr_kind_tag",
      "Input"  -> {1.5},
      "Output" -> "Real",
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-real"|>,

    (* Same wstp/master limitation as echo_expr above: no BigInteger/BigReal
       detection, so these no longer round-trip as their "Big" kind. *)
    <|"Export" -> "expr_kind_tag",
      "Input"  -> {2^200},
      "Output" -> _LibraryFunctionError,
      "Messages" -> {_},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-big_integer"|>,

    <|"Export" -> "expr_kind_tag",
      "Input"  -> {N[Pi, 50]},
      "Output" -> "Real",
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-big_real"|>,

    <|"Export" -> "expr_kind_tag",
      "Input"  -> {"hello"},
      "Output" -> "String",
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-string"|>,

    <|"Export" -> "expr_kind_tag",
      "Input"  -> {Pi},
      "Output" -> "Symbol",
      "Messages" -> {},
      "TestID" -> "Examples-legacy_wstp-expr_kind_tag-symbol"|>,

    (* ── wxf: echo_point ──────────────────────────────────────────────────────── *)

    <|"Export" -> "wxf_echo_point",
      "Input"  -> {{1, 2}},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-wxf-echo_point-list"|>,

    <|"Export" -> "wxf_echo_point",
      "Input"  -> {ByteArray[{1, 2}]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-wxf-echo_point-byte_array"|>,

    <|"Export" -> "wxf_echo_point",
      "Input"  -> {NumericArray[{1, 2}]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-wxf-echo_point-numeric_array_u8"|>,

    <|"Export" -> "wxf_echo_point",
      "Input"  -> {NumericArray[{1, 2}, "Integer32"]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-wxf-echo_point-numeric_array_i32"|>,

    <|"Export" -> "wxf_echo_point",
      "Input"  -> {<|"x" -> 1, "y" -> 2|>},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-wxf-echo_point-assoc_xy"|>,

    <|"Export" -> "wxf_echo_point",
      "Input"  -> {<|"y" -> 2, "x" -> 1|>},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-wxf-echo_point-assoc_yx"|>,

    <|"Export" -> "wxf_echo_point",
      "Input"  -> {Hold[1, 2]},
      "Output" -> <|"x" -> 1., "y" -> 2.|>,
      "Messages" -> {},
      "TestID" -> "Examples-wxf-echo_point-hold"|>,

    <|"Export" -> "wxf_echo_point",
      "Input"  -> {{1}},
      "Output" -> _Failure,
      "Messages" -> {},
      "TestID" -> "Examples-wxf-echo_point-wrong_length"|>,

    <|"Export" -> "wxf_echo_point",
      "Input"  -> {"hello"},
      "Output" -> _Failure,
      "Messages" -> {},
      "TestID" -> "Examples-wxf-echo_point-wrong_type"|>,

    (* ── margs: raw MArgument functions, annotated with args/ret ─────────────── *)

    <|"Export" -> "margs_add",
      "Input"  -> {2., 3.},
      "Output" -> 5.,
      "Messages" -> {},
      "TestID" -> "Examples-margs-add"|>,

    <|"Export" -> "margs_dot",
      "Input"  -> {NumericArray[{1., 2., 3.}, "Real64"], NumericArray[{4., 5., 6.}, "Real64"]},
      "Output" -> 32.,
      "Messages" -> {},
      "TestID" -> "Examples-margs-dot"|>,

    <|"Export" -> "margs_scale_array",
      "Input"  -> {NumericArray[{1., 2., 3.}, "Real64"], 2.},
      "Output" -> NumericArray[{2., 4., 6.}, "Real64"],
      "Messages" -> {},
      "TestID" -> "Examples-margs-scale_array"|>,

    <|"Export" -> "margs_sparse_array_merge",
      "Input"  -> {
        SparseArray[{1 -> 1., 3 -> 3.}, 5],
        SparseArray[{2 -> 2., 3 -> 30.}, 5]
      },
      "Output" -> SparseArray[{1., 2., 30., 0., 0.}],
      "Messages" -> {},
      "TestID" -> "Examples-margs-sparse_array_merge"|>,

    (* ── panic tests ─────────────────────────────────────────────────────────── *)

    <|"Export" -> "native_force_panic",
      "Input"  -> {42.0},
      "Output" -> _LibraryFunctionError,
      "Messages" -> {_},
      "TestID" -> "Examples-native-force_panic"|>,

    <|"Export" -> "margs_force_panic",
      "Input"  -> {42.0},
      "Output" -> _LibraryFunctionError,
      "Messages" -> {_},
      "TestID" -> "Examples-margs-force_panic"|>,

    <|"Export" -> "wstp_force_panic",
      "Input"  -> {42.0},
      "Output" -> _Failure,
      "Messages" -> {},
      "TestID" -> "Examples-wstp-force_panic"|>,

    <|"Export" -> "wxf_force_panic",
      "Input"  -> {42.0},
      "Output" -> _Failure,
      "Messages" -> {},
      "TestID" -> "Examples-wxf-force_panic"|>

};

(* ── Runner ──────────────────────────────────────────────────────────────────── *)

TestCreate[
    Apply[$Libs[#Export], #Input],
    #Output,
    #Messages,
    SameTest -> MatchQ,
    TestID -> #TestID
] & /@ $Tests
