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
      "TestID" -> "Examples-wxf-force_panic"|>,

    (* ── vendor-chrono: chrono_add_seconds ───────────────────────────────────── *)
    (* `DateTime<Utc>` round-trips as `DateObject[{y,m,d,h,mi,s}, "Instant",
       "Gregorian", "UTC"]` via wolfram-expr's `ViaWXF` bridge (see
       wolfram-serialize/src/vendor/chrono.rs) — no library-specific WL glue
       needed, an ordinary `DateObject` is sent and received. Seconds must be
       sent as explicit reals (`0.`, not `0`): the wire's seconds slot is `f64`
       and WXF doesn't widen `Integer` -> `Real`.

       The timezone slot is matched as `"UTC" | 0.`, not a bare `"UTC"`: once
       any real named zone (e.g. the vendor-chrono-tz tests below) has been
       evaluated anywhere in this kernel session, the kernel's own `DateObject`
       starts normalizing `"UTC"` to the numeric offset `0.` for everything
       evaluated afterward — including our library's return value, which
       TestReport evaluates lazily, after every DateObject literal in this
       file has already run once. Both forms name the same zone, so accept
       either. *)

    <|"Export" -> "chrono_add_seconds",
      "Input"  -> {DateObject[{2024, 1, 1, 0, 0, 0.}, "Instant", "Gregorian", "UTC"], 30.},
      "Output" -> DateObject[{2024, 1, 1, 0, 0, 30.}, "Instant", "Gregorian", "UTC" | 0.],
      "Messages" -> {},
      "TestID" -> "Examples-vendor_chrono-add_seconds-whole"|>,

    <|"Export" -> "chrono_add_seconds",
      "Input"  -> {DateObject[{2024, 1, 1, 23, 59, 59.}, "Instant", "Gregorian", "UTC"], 1.5},
      "Output" -> DateObject[{2024, 1, 2, 0, 0, 0.5}, "Instant", "Gregorian", "UTC" | 0.],
      "Messages" -> {},
      "TestID" -> "Examples-vendor_chrono-add_seconds-fractional_carries_day"|>,

    <|"Export" -> "chrono_add_seconds",
      "Input"  -> {DateObject[{2024, 1, 1, 0, 0, 10.}, "Instant", "Gregorian", "UTC"], -10.},
      "Output" -> DateObject[{2024, 1, 1, 0, 0, 0.}, "Instant", "Gregorian", "UTC" | 0.],
      "Messages" -> {},
      "TestID" -> "Examples-vendor_chrono-add_seconds-negative"|>,

    <|"Export" -> "chrono_add_seconds",
      "Input"  -> {DateObject[{2024, 1, 1, 0, 0, 0.}, "Instant", "Gregorian", "UTC"], "not a number"},
      "Output" -> _Failure,
      "Messages" -> {},
      "TestID" -> "Examples-vendor_chrono-add_seconds-wrong_type"|>,

    (* 2 days, 3 hours, 12 seconds = 183612 s — large enough to roll the day
       past the end of the month (and, in the second case, past the end of
       the year), verifying carry propagates through day/month/year, not just
       the seconds slot. *)

    <|"Export" -> "chrono_add_seconds",
      "Input"  -> {DateObject[{2024, 1, 30, 23, 0, 0.}, "Instant", "Gregorian", "UTC"], 183612.},
      "Output" -> DateObject[{2024, 2, 2, 2, 0, 12.}, "Instant", "Gregorian", "UTC" | 0.],
      "Messages" -> {},
      "TestID" -> "Examples-vendor_chrono-add_seconds-rolls_over_month"|>,

    <|"Export" -> "chrono_add_seconds",
      "Input"  -> {DateObject[{2024, 12, 30, 23, 0, 0.}, "Instant", "Gregorian", "UTC"], 183612.},
      "Output" -> DateObject[{2025, 1, 2, 2, 0, 12.}, "Instant", "Gregorian", "UTC" | 0.],
      "Messages" -> {},
      "TestID" -> "Examples-vendor_chrono-add_seconds-rolls_over_year"|>,

    (* `DateObject["TimeZone" -> "UTC"]` (like `DateObject[]`/`Now`) carries the
       current instant with a *precision-annotated* seconds value, e.g.
       `34.358312`8.29`, not a plain machine real. On the wire that's a
       `BigReal` token, not `Real64` — the seconds slot's `f64` bridge
       (wolfram-serialize/src/vendor/chrono.rs) narrows it to a machine double
       the same way reading a `BigReal` over WSTP does (see the legacy_wstp
       big_real case above), rather than rejecting it outright. Since "now" is
       different every run, this only checks the call succeeds and returns a
       `DateObject`, not an exact value. *)
    <|"Export" -> "chrono_add_seconds",
      "Input"  -> {DateObject["TimeZone" -> "UTC"], 30.},
      "Output" -> _DateObject,
      "Messages" -> {},
      "TestID" -> "Examples-vendor_chrono-add_seconds-current_time_precision_real"|>,

    (* ── vendor-chrono-tz: chrono_tz_add_seconds ─────────────────────────────── *)
    (* Same bridge, but the timezone slot carries an IANA zone name instead of
       "UTC"/a numeric offset (see wolfram-serialize/src/vendor/chrono_tz.rs).
       The result keeps the same named zone, so its wall-clock offset can
       differ before and after the add across a DST transition. *)

    <|"Export" -> "chrono_tz_add_seconds",
      "Input"  -> {DateObject[{2026, 4, 14, 9, 30, 0.}, "Instant", "Gregorian", "America/Chicago"], 30.},
      "Output" -> DateObject[{2026, 4, 14, 9, 30, 30.}, "Instant", "Gregorian", "America/Chicago"],
      "Messages" -> {},
      "TestID" -> "Examples-vendor_chrono_tz-add_seconds-whole"|>,

    (* 2025-03-09 is the US spring-forward date: clocks jump 02:00 -> 03:00
       local (CST -> CDT) at 08:00 UTC. Adding exactly 1 hour of elapsed time
       to 01:30 CST lands at 03:30 CDT — the wall clock jumps 2 hours even
       though only 1 hour actually passed, because the offset itself shifted
       by an hour partway through. *)

    <|"Export" -> "chrono_tz_add_seconds",
      "Input"  -> {DateObject[{2025, 3, 9, 1, 30, 0.}, "Instant", "Gregorian", "America/Chicago"], 3600.},
      "Output" -> DateObject[{2025, 3, 9, 3, 30, 0.}, "Instant", "Gregorian", "America/Chicago"],
      "Messages" -> {},
      "TestID" -> "Examples-vendor_chrono_tz-add_seconds-crosses_spring_forward"|>,

    <|"Export" -> "chrono_tz_add_seconds",
      "Input"  -> {DateObject[{2026, 4, 14, 9, 30, 0.}, "Instant", "Gregorian", "Not/AZone"], 30.},
      "Output" -> _Failure,
      "Messages" -> {},
      "TestID" -> "Examples-vendor_chrono_tz-add_seconds-unknown_zone"|>

};

(* ── Runner ──────────────────────────────────────────────────────────────────── *)

TestCreate[
    Apply[$Libs[#Export], #Input],
    #Output,
    #Messages,
    SameTest -> MatchQ,
    TestID -> #TestID
] & /@ $Tests
