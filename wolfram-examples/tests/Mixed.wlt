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
      "TestID" -> "Examples-mixed-dot"|>

};

(* ── Runner ──────────────────────────────────────────────────────────────────── *)

TestCreate[
    Apply[$Libs[#Export], #Input],
    #Output,
    #Messages,
    SameTest -> MatchQ,
    TestID -> #TestID
] & /@ $Tests
