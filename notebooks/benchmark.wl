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

(* `mem_reset`/`mem_allocated` read a byte counter kept by a tracking global
   allocator wired into the wolfram-examples-internal dylib (src/mem.rs) —
   it tallies every byte requested via the Rust allocator, letting us measure
   allocation churn per call the same way avgUs/timeMicros measure wall time. *)
memReset      = $fns["mem_reset"];
memAllocated  = $fns["mem_allocated"];

(* Fail loudly instead of silently benchmarking a Missing[KeyAbsent, ...] stub —
   that stub "returns" without doing any work, so timing it makes whatever it's
   being compared against look arbitrarily slow. *)
missingFns = Select[
  {"nativeAdd" -> nativeAdd, "nativeDot" -> nativeDot, "nativeScale" -> nativeScale,
   "margsAdd" -> margsAdd, "margsDot" -> margsDot, "margsScale" -> margsScale,
   "wstpAdd" -> wstpAdd, "wstpDot" -> wstpDot, "wstpScale" -> wstpScale, "wstpDup" -> wstpDup,
   "wxfAdd" -> wxfAdd, "wxfDot" -> wxfDot, "wxfScale" -> wxfScale, "wxfDup" -> wxfDup,
   "wxfPoint" -> wxfPoint, "wxfDs" -> wxfDs,
   "memReset" -> memReset, "memAllocated" -> memAllocated},
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

(* Bytes allocated (via the Rust global allocator, see mem_reset/mem_allocated
   above), averaged over `reps` calls. Unlike wall time, an allocation count is
   exact rather than noisy, so `reps` here just amortizes any one-time
   allocator warmup (e.g. first-call arena growth) rather than fighting jitter. *)
SetAttributes[avgBytes, HoldFirst];
avgBytes[expr_, reps_ : 200] := (memReset[]; Do[expr, reps]; N[memAllocated[]]/reps);

bytesPerCall[fn_, reps_] := (memReset[]; Do[fn[], reps]; N[memAllocated[]]/reps);

fmtUs[x_] := ToString[NumberForm[x, {5, 2}]] <> " \[Mu]s";
fmtRatio[x_] := ToString[NumberForm[x, {4, 2}]] <> "x";
fmtBytes[x_] := Which[
  x < 1024,      ToString[NumberForm[x, {5, 1}]] <> " B",
  x < 1024^2,    ToString[NumberForm[x/1024, {5, 2}]] <> " KB",
  True,          ToString[NumberForm[x/1024^2, {5, 2}]] <> " MB"
];

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

(* Linear, not log, scale: native/margs calls on scalar-returning ops (e.g.
   dot) are true zero-copy and allocate exactly 0 bytes, which a log axis
   can't represent. *)
lineOptsBytes[title_, styles_] := {
  PlotLabel  -> Style[title, Bold, 13],
  Frame -> True,
  FrameLabel -> {{"bytes / call", None}, {"n", None}},
  PlotStyle  -> styles,
  Joined -> True, Mesh -> All, MeshStyle -> PointSize[0.018],
  GridLines -> Automatic, GridLinesStyle -> LightGray,
  ImageSize -> 500, ImagePadding -> {{65, 140}, {40, 20}}};

barOpts[title_, colors_, labels_] := {
  PlotLabel  -> Style[title, Bold, 13],
  ChartStyle -> colors,
  ChartLabels -> Placed[labels, Below],
  Frame -> {{True, False}, {True, False}},
  FrameLabel -> {{"\[Mu]s / call", None}, {None, None}},
  GridLines -> {None, Automatic}, GridLinesStyle -> LightGray,
  BarSpacing -> 0.4, ImageSize -> 400, ImagePadding -> {{55, 10}, {50, 30}}};

barOptsBytes[title_, colors_, labels_] := {
  PlotLabel  -> Style[title, Bold, 13],
  ChartStyle -> colors,
  ChartLabels -> Placed[labels, Below],
  Frame -> {{True, False}, {True, False}},
  FrameLabel -> {{"bytes / call", None}, {None, None}},
  GridLines -> {None, Automatic}, GridLinesStyle -> LightGray,
  BarSpacing -> 0.4, ImageSize -> 400, ImagePadding -> {{65, 10}, {50, 30}}};

mkLegend[labels_, colors_] := LineLegend[colors, labels,
  LegendMarkerSize -> 14, LegendFunction -> "Frame"];

ns = {10, 100, 1000, 10000, 100000};

(* ══════════════════════════════════════════════════════════════════════════ *)
(* add                                                                        *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["\n=== add(a, b) ==="];
Module[{tN, tM, tW, tX, mN, mM, mW, mX},
  tN = avgUs[nativeAdd[3., 4.]];
  tM = avgUs[margsAdd[3., 4.]];
  tW = avgUs[wstpAdd[3., 4.]];
  tX = avgUs[wxfAdd[3., 4.]];
  mN = avgBytes[nativeAdd[3., 4.]];
  mM = avgBytes[margsAdd[3., 4.]];
  mW = avgBytes[wstpAdd[3., 4.]];
  mX = avgBytes[wxfAdd[3., 4.]];
  printTable[
    {"impl", "time", "vs native", "vs wstp", "bytes/call"},
    {{"native", fmtUs[tN], "1.00x", fmtRatio[tN/tW], fmtBytes[mN]},
     {"margs",  fmtUs[tM], fmtRatio[tM/tN], fmtRatio[tM/tW], fmtBytes[mM]},
     {"wstp",   fmtUs[tW], fmtRatio[tW/tN], "1.00x", fmtBytes[mW]},
     {"wxf",    fmtUs[tX], fmtRatio[tX/tN], fmtRatio[tX/tW], fmtBytes[mX]}}
  ];
  Print @ BarChart[
    {tN, tM, tW, tX},
    Sequence @@ barOpts["add(a, b)  -  time", {nC, mC, wC, xC}, {"native", "margs", "wstp", "wxf"}]];
  Print @ BarChart[
    {mN, mM, mW, mX},
    Sequence @@ barOptsBytes["add(a, b)  -  memory", {nC, mC, wC, xC}, {"native", "margs", "wstp", "wxf"}]];
];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* duplicate                                                                  *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["\n=== duplicate(x) ==="];
Module[{tW, tX, mW, mX},
  tW = avgUs[wstpDup[42]];
  tX = avgUs[wxfDup[42]];
  mW = avgBytes[wstpDup[42]];
  mX = avgBytes[wxfDup[42]];
  printTable[
    {"impl", "time", "vs wstp", "bytes/call"},
    {{"wstp", fmtUs[tW], "1.00x", fmtBytes[mW]},
     {"wxf",  fmtUs[tX], fmtRatio[tX/tW], fmtBytes[mX]}}
  ];
  Print @ BarChart[
    {tW, tX},
    Sequence @@ barOpts["duplicate(x)  -  time", {wC, xC}, {"wstp", "wxf"}]];
  Print @ BarChart[
    {mW, mX},
    Sequence @@ barOptsBytes["duplicate(x)  -  memory", {wC, xC}, {"wstp", "wxf"}]];
];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* echo_point                                                                 *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["\n=== echo_point(p) ==="];
Module[{tX, mX},
  tX = avgUs[wxfPoint[<|"x" -> 1.5, "y" -> 2.5|>]];
  mX = avgBytes[wxfPoint[<|"x" -> 1.5, "y" -> 2.5|>]];
  printTable[{"impl", "time", "bytes/call"}, {{"wxf", fmtUs[tX], fmtBytes[mX]}}];
  Print @ BarChart[{tX}, Sequence @@ barOpts["echo_point(p)  -  time", {xC}, {"wxf"}]];
  Print @ BarChart[{mX}, Sequence @@ barOptsBytes["echo_point(p)  -  memory", {xC}, {"wxf"}]];
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
     timeMicros[Function[Module[{j=nextI[]}, wxfDot[as[[j]], bs[[j]]]]], r],
     (idx = 0; bytesPerCall[Function[Module[{j=nextI[]}, nativeDot[as[[j]], bs[[j]]]]], r]),
     (idx = 0; bytesPerCall[Function[Module[{j=nextI[]}, margsDot[as[[j]], bs[[j]]]]], r]),
     (idx = 0; bytesPerCall[Function[Module[{j=nextI[]}, wstpDot[as[[j]], bs[[j]]]]], r]),
     (idx = 0; bytesPerCall[Function[Module[{j=nextI[]}, wxfDot[as[[j]], bs[[j]]]]], r])}],
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

Print["\n=== dot(a, b)  -  bytes vs n ==="];
printTable[
  {"n", "native", "margs", "wstp", "wxf", "wxf/wstp"},
  Map[{ToString[#[[1]]], fmtBytes[#[[6]]], fmtBytes[#[[7]]], fmtBytes[#[[8]]], fmtBytes[#[[9]]], fmtRatio[#[[9]]/#[[8]]]} &, dotRows]
];
Print @ Legended[
  ListLinePlot[{dotRows[[All,{1,6}]], dotRows[[All,{1,7}]], dotRows[[All,{1,8}]], dotRows[[All,{1,9}]]},
    Sequence @@ lineOptsBytes["dot(a, b)  -  bytes vs n",
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
     timeMicros[Function[Module[{j=nextI[]}, Total @ Normal @ wxfScale[as[[j]], 2.]]], r],
     (idx = 0; bytesPerCall[Function[Module[{j=nextI[]}, Total @ nativeScale[as[[j]], 2.]]], r]),
     (idx = 0; bytesPerCall[Function[Module[{j=nextI[]}, Total @ margsScale[as[[j]], 2.]]], r]),
     (idx = 0; bytesPerCall[Function[Module[{j=nextI[]}, Total @ Normal @ wstpScale[as[[j]], 2.]]], r]),
     (idx = 0; bytesPerCall[Function[Module[{j=nextI[]}, Total @ Normal @ wxfScale[as[[j]], 2.]]], r])}],
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

Print["\n=== scale_array(arr, f)  -  bytes vs n ==="];
printTable[
  {"n", "native", "margs", "wstp", "wxf", "wxf/wstp"},
  Map[{ToString[#[[1]]], fmtBytes[#[[6]]], fmtBytes[#[[7]]], fmtBytes[#[[8]]], fmtBytes[#[[9]]], fmtRatio[#[[9]]/#[[8]]]} &, scRows]
];
Print @ Legended[
  ListLinePlot[{scRows[[All,{1,6}]], scRows[[All,{1,7}]], scRows[[All,{1,8}]], scRows[[All,{1,9}]]},
    Sequence @@ lineOptsBytes["scale_array(arr, f)  -  bytes vs n",
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
    {n, timeMicros[Function[wxfDs[ds]], r], bytesPerCall[Function[wxfDs[ds]], r]}],
  {n, ns}];
printTable[
  {"n", "wxf"},
  Map[{ToString[#[[1]]], fmtUs[#[[2]]]} &, dsRows]
];
Print @ Legended[
  ListLinePlot[{dsRows[[All,{1,2}]]},
    Sequence @@ lineOpts["echo_dataset(ds)  -  \[Mu]s vs n",
      {Directive[xC, Thick]}]],
  mkLegend[{"wxf"}, {xC}]];

Print["\n=== echo_dataset(ds)  -  bytes vs n ==="];
printTable[
  {"n", "wxf"},
  Map[{ToString[#[[1]]], fmtBytes[#[[3]]]} &, dsRows]
];
Print @ Legended[
  ListLinePlot[{dsRows[[All,{1,3}]]},
    Sequence @@ lineOptsBytes["echo_dataset(ds)  -  bytes vs n",
      {Directive[xC, Thick]}]],
  mkLegend[{"wxf"}, {xC}]];

Print["\nDone."];
