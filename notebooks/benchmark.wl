(* ── Build ──────────────────────────────────────────────────────────────── *)
repo  = FileNameJoin[{DirectoryName[$InputFileName], ".."}];
cargo = FileNameJoin[{$HomeDirectory, ".cargo", "bin", "cargo"}];

(* `--features wstp` is required or the `wstp`/`legacy_wstp` modules are
   silently skipped by Cargo's required-features gate (wstp-sys needs a WSTP SDK,
   so it's opt-in). Without it, $fns["wstp_..."] below resolves to a
   Missing[KeyAbsent, ...] stub that "returns" instantly without doing any work —
   which used to make WXF look catastrophically slower than WSTP in this
   benchmark, when in fact WSTP just wasn't running at all.

   Uses `wolfram-examples-internal` (the in-repo test/benchmark crate, built via
   local path deps) rather than the standalone `wolfram-examples` workspace —
   that one only holds copy-paste sample crates (duckdb, math) and depends on
   published crates.io releases, not this checkout's code. Functions here have
   no namespace prefix (namespace-exports is off); native_/wstp_/wxf_ prefixes
   in the Rust source disambiguate instead. *)
libDir = StringTrim @ First @ ExternalEvaluate[
  {"Shell", "ProcessDirectory" -> repo, "ReturnType" -> "StandardOutput"},
  {cargo -> {"wl", "build", "--release", "-p", "wolfram-examples-internal", "--features", "wstp"}}
];

(* ── Load ───────────────────────────────────────────────────────────────── *)
$fns = Get[FileNameJoin[{libDir, "Functions.wl"}]];

nativeAdd   = $fns["native_add"];
nativeDot   = $fns["native_dot"];
nativeScale = $fns["native_scale_array"];
margsAdd    = $fns["margs_add"];
margsDot    = $fns["margs_dot"];
margsScale  = $fns["margs_scale_array"];
wstpAdd     = $fns["wstp_add"];
wstpDot     = $fns["wstp_dot"];
wstpScale   = $fns["wstp_scale_array"];
wstpDup     = $fns["wstp_duplicate"];
wxfAdd      = $fns["wxf_add"];
wxfDot      = $fns["wxf_dot"];
wxfScale    = $fns["wxf_scale_array"];
wxfDup      = $fns["wxf_duplicate"];
wxfPoint    = $fns["wxf_echo_point"];
wxfDs       = $fns["wxf_echo_dataset"];

(* Fail loudly instead of silently benchmarking a Missing[KeyAbsent, ...] stub —
   that stub "returns" without doing any work, so timing it makes whatever it's
   being compared against look arbitrarily slow. *)
missingFns = Select[
  {"nativeAdd" -> nativeAdd, "nativeDot" -> nativeDot, "nativeScale" -> nativeScale,
   "margsAdd" -> margsAdd, "margsDot" -> margsDot, "margsScale" -> margsScale,
   "wstpAdd" -> wstpAdd, "wstpDot" -> wstpDot, "wstpScale" -> wstpScale, "wstpDup" -> wstpDup,
   "wxfAdd" -> wxfAdd, "wxfDot" -> wxfDot, "wxfScale" -> wxfScale, "wxfDup" -> wxfDup,
   "wxfPoint" -> wxfPoint, "wxfDs" -> wxfDs},
  MatchQ[#[[2]], _Missing] &
];
If[missingFns =!= {},
  Print["ERROR: functions not found in Functions.wl: ", Keys[missingFns]];
  Quit[1];
];

(* ── Helpers ─────────────────────────────────────────────────────────────── *)
nC = RGBColor["#2196F3"]; wC = RGBColor["#FF5722"]; xC = RGBColor["#4CAF50"];
mC = RGBColor["#9C27B0"];

rotN = 32; idx = 0; nextI[] := (idx = Mod[idx, rotN] + 1; idx);
mkNA[n_] := Table[NumericArray[RandomReal[1, n], "Real64"], rotN];

SetAttributes[avgUs, HoldFirst];
avgUs[expr_] := RepeatedTiming[expr, 1][[1]] * 1*^6;

timeMicros[fn_, reps_] := Module[{s = 0, t},
  t = AbsoluteTiming[Do[s += fn[], reps]][[1]]; t/reps*1*^6];

fmtUs[x_] := ToString[NumberForm[x, {5, 2}]] <> " \[Mu]s";
fmtRatio[x_] := ToString[NumberForm[x, {4, 2}]] <> "x";

(* Print a simple left-aligned text table: header row + data rows, each a
   list of already-stringified cells. Column widths derive from content. *)
printTable[header_List, rows_List] := Module[{cols, widths, pad, line},
  cols = Transpose[{header}~Join~rows];
  widths = Max[StringLength /@ #] & /@ cols;
  pad[s_, w_] := StringPadRight[s, w + 2];
  line[r_] := StringJoin @ MapThread[pad, {r, widths}];
  Print[line[header]];
  Print[StringJoin @ Table["-", {Total[widths] + 2*Length[widths]}]];
  Print[line[#]] & /@ rows;
];

lineOpts[title_, styles_] := {
  PlotLabel  -> Style[title, Bold, 13],
  Frame -> True,
  FrameLabel -> {{"time (\[Mu]s)", None}, {"n", None}},
  PlotStyle  -> styles,
  Joined -> True, Mesh -> All, MeshStyle -> PointSize[0.018],
  GridLines -> Automatic, GridLinesStyle -> LightGray,
  ImageSize -> 500, ImagePadding -> {{55, 140}, {40, 20}}};

barOpts[title_, colors_, labels_] := {
  PlotLabel  -> Style[title, Bold, 13],
  ChartStyle -> colors,
  ChartLabels -> Placed[labels, Below],
  Frame -> {{True, False}, {True, False}},
  FrameLabel -> {{"\[Mu]s / call", None}, {None, None}},
  GridLines -> {None, Automatic}, GridLinesStyle -> LightGray,
  BarSpacing -> 0.4, ImageSize -> 400, ImagePadding -> {{55, 10}, {50, 30}}};

mkLegend[labels_, colors_] := LineLegend[colors, labels,
  LegendMarkerSize -> 14, LegendFunction -> "Frame"];

ns = {10, 100, 1000, 10000, 100000};

(* ══════════════════════════════════════════════════════════════════════════ *)
(* add                                                                        *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["\n=== add(a, b) ==="];
Module[{tN, tM, tW, tX},
  tN = avgUs[nativeAdd[3., 4.]];
  tM = avgUs[margsAdd[3., 4.]];
  tW = avgUs[wstpAdd[3., 4.]];
  tX = avgUs[wxfAdd[3., 4.]];
  printTable[
    {"impl", "time", "vs native", "vs wstp"},
    {{"native", fmtUs[tN], "1.00x", fmtRatio[tN/tW]},
     {"margs",  fmtUs[tM], fmtRatio[tM/tN], fmtRatio[tM/tW]},
     {"wstp",   fmtUs[tW], fmtRatio[tW/tN], "1.00x"},
     {"wxf",    fmtUs[tX], fmtRatio[tX/tN], fmtRatio[tX/tW]}}
  ];
  Print @ BarChart[
    {tN, tM, tW, tX},
    Sequence @@ barOpts["add(a, b)", {nC, mC, wC, xC}, {"native", "margs", "wstp", "wxf"}]];
];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* duplicate                                                                  *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["\n=== duplicate(x) ==="];
Module[{tW, tX},
  tW = avgUs[wstpDup[42]];
  tX = avgUs[wxfDup[42]];
  printTable[
    {"impl", "time", "vs wstp"},
    {{"wstp", fmtUs[tW], "1.00x"},
     {"wxf",  fmtUs[tX], fmtRatio[tX/tW]}}
  ];
  Print @ BarChart[
    {tW, tX},
    Sequence @@ barOpts["duplicate(x)", {wC, xC}, {"wstp", "wxf"}]];
];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* echo_point                                                                 *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["\n=== echo_point(p) ==="];
Module[{tX},
  tX = avgUs[wxfPoint[<|"x" -> 1.5, "y" -> 2.5|>]];
  printTable[{"impl", "time"}, {{"wxf", fmtUs[tX]}}];
  Print @ BarChart[{tX}, Sequence @@ barOpts["echo_point(p)", {xC}, {"wxf"}]];
];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* dot                                                                        *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["\n=== dot(a, b)  -  \[Mu]s vs n ==="];
dotRows = Table[
  Module[{as = mkNA[n], bs = mkNA[n], r = Max[1, Round[4000/n*100]]},
    idx = 0;
    {n,
     timeMicros[Function[Module[{j=nextI[]}, nativeDot[as[[j]], bs[[j]]]]], r],
     timeMicros[Function[Module[{j=nextI[]}, margsDot[as[[j]], bs[[j]]]]], r],
     timeMicros[Function[Module[{j=nextI[]}, wstpDot[as[[j]], bs[[j]]]]], r],
     timeMicros[Function[Module[{j=nextI[]}, wxfDot[as[[j]], bs[[j]]]]], r]}],
  {n, ns}];
printTable[
  {"n", "native", "margs", "wstp", "wxf", "wxf/wstp"},
  Map[{ToString[#[[1]]], fmtUs[#[[2]]], fmtUs[#[[3]]], fmtUs[#[[4]]], fmtUs[#[[5]]], fmtRatio[#[[5]]/#[[4]]]} &, dotRows]
];
Print @ Legended[
  ListLinePlot[{dotRows[[All,{1,2}]], dotRows[[All,{1,3}]], dotRows[[All,{1,4}]], dotRows[[All,{1,5}]]},
    Sequence @@ lineOpts["dot(a, b)  -  \[Mu]s vs n",
      {Directive[nC,Thick], Directive[mC,Thick], Directive[wC,Thick], Directive[xC,Thick]}]],
  mkLegend[{"native","margs","wstp","wxf"}, {nC, mC, wC, xC}]];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* scale_array                                                                *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["\n=== scale_array(arr, f)  -  \[Mu]s vs n ==="];
scRows = Table[
  Module[{as = mkNA[n], r = Max[1, Round[4000/n*100]]},
    idx = 0;
    {n,
     timeMicros[Function[Module[{j=nextI[]}, Total @ nativeScale[as[[j]], 2.]]], r],
     timeMicros[Function[Module[{j=nextI[]}, Total @ margsScale[as[[j]], 2.]]], r],
     timeMicros[Function[Module[{j=nextI[]}, Total @ Normal @ wstpScale[as[[j]], 2.]]], r],
     timeMicros[Function[Module[{j=nextI[]}, Total @ Normal @ wxfScale[as[[j]], 2.]]], r]}],
  {n, ns}];
printTable[
  {"n", "native", "margs", "wstp", "wxf", "wxf/wstp"},
  Map[{ToString[#[[1]]], fmtUs[#[[2]]], fmtUs[#[[3]]], fmtUs[#[[4]]], fmtUs[#[[5]]], fmtRatio[#[[5]]/#[[4]]]} &, scRows]
];
Print @ Legended[
  ListLinePlot[{scRows[[All,{1,2}]], scRows[[All,{1,3}]], scRows[[All,{1,4}]], scRows[[All,{1,5}]]},
    Sequence @@ lineOpts["scale_array(arr, f)  -  \[Mu]s vs n",
      {Directive[nC,Thick], Directive[mC,Thick], Directive[wC,Thick], Directive[xC,Thick]}]],
  mkLegend[{"native","margs","wstp","wxf"}, {nC, mC, wC, xC}]];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* echo_dataset                                                               *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["\n=== echo_dataset(ds)  -  \[Mu]s vs n ==="];
dsRows = Table[
  Module[{r = Max[1, Round[4000/n*100]],
          ds = <|"name"   -> "t",
                 "blob"   -> ByteArray[RandomInteger[{0, 255}, n]],
                 "values" -> NumericArray[RandomReal[1, n], "Real64"]|>},
    {n, timeMicros[Function[wxfDs[ds]], r]}],
  {n, ns}];
printTable[
  {"n", "wxf"},
  Map[{ToString[#[[1]]], fmtUs[#[[2]]]} &, dsRows]
];
Print @ Legended[
  ListLinePlot[{dsRows},
    Sequence @@ lineOpts["echo_dataset(ds)  -  \[Mu]s vs n",
      {Directive[xC, Thick]}]],
  mkLegend[{"wxf"}, {xC}]];

Print["\nDone."];
