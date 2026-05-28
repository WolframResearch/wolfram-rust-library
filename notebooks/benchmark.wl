(* ── Build ──────────────────────────────────────────────────────────────── *)
repo  = FileNameJoin[{DirectoryName[$InputFileName], ".."}];
cargo = FileNameJoin[{$HomeDirectory, ".cargo", "bin", "cargo"}];

libDir = StringTrim @ First @ ExternalEvaluate[
  {"Shell", "ProcessDirectory" -> repo, "ReturnType" -> "StandardOutput"},
  {cargo -> {"wl", "build", "--release", "--examples", "-p", "wolfram-examples"}}
];

(* ── Load ───────────────────────────────────────────────────────────────── *)
$fns = Get[FileNameJoin[{libDir, "Functions.wl"}]];

nativeAdd   = $fns["types_native::add"];
nativeDot   = $fns["types_native::dot"];
nativeScale = $fns["types_native::scale_array"];
wstpAdd     = $fns["types_wstp::add"];
wstpDot     = $fns["types_wstp::dot"];
wstpScale   = $fns["types_wstp::scale_array"];
wstpDup     = $fns["types_wstp::duplicate"];
wxfAdd      = $fns["types_wxf::add"];
wxfDot      = $fns["types_wxf::dot"];
wxfScale    = $fns["types_wxf::scale_array"];
wxfDup      = $fns["types_wxf::duplicate"];
wxfPoint    = $fns["types_wxf::echo_point"];
wxfDs       = $fns["types_wxf::echo_dataset"];

(* ── Helpers ─────────────────────────────────────────────────────────────── *)
nC = RGBColor["#2196F3"]; wC = RGBColor["#FF5722"]; xC = RGBColor["#4CAF50"];

rotN = 32; idx = 0; nextI[] := (idx = Mod[idx, rotN] + 1; idx);
mkNA[n_] := Table[NumericArray[RandomReal[1, n], "Real64"], rotN];

SetAttributes[avgUs, HoldFirst];
avgUs[expr_] := RepeatedTiming[expr, 1][[1]] * 1*^6;

timeMicros[fn_, reps_] := Module[{s = 0, t},
  t = AbsoluteTiming[Do[s += fn[], reps]][[1]]; t/reps*1*^6];

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
Print["Benchmarking add..."];
Print @ BarChart[
  {avgUs[nativeAdd[3., 4.]], avgUs[wstpAdd[3., 4.]], avgUs[wxfAdd[3., 4.]]},
  Sequence @@ barOpts["add(a, b)", {nC, wC, xC}, {"native", "wstp", "wxf"}]];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* duplicate                                                                  *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["Benchmarking duplicate..."];
Print @ BarChart[
  {avgUs[wstpDup[42]], avgUs[wxfDup[42]]},
  Sequence @@ barOpts["duplicate(x)", {wC, xC}, {"wstp", "wxf"}]];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* echo_point                                                                 *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["Benchmarking echo_point..."];
Print @ BarChart[
  {avgUs[wxfPoint[<|"x" -> 1.5, "y" -> 2.5|>]]},
  Sequence @@ barOpts["echo_point(p)", {xC}, {"wxf"}]];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* dot                                                                        *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["Benchmarking dot..."];
dotRows = Table[
  Module[{as = mkNA[n], bs = mkNA[n], r = Max[1, Round[4000/n*100]]},
    idx = 0;
    {n,
     timeMicros[Function[Module[{j=nextI[]}, nativeDot[as[[j]], bs[[j]]]]], r],
     timeMicros[Function[Module[{j=nextI[]}, wstpDot[as[[j]], bs[[j]]]]], r],
     timeMicros[Function[Module[{j=nextI[]}, wxfDot[as[[j]], bs[[j]]]]], r]}],
  {n, ns}];
Print @ Legended[
  ListLinePlot[{dotRows[[All,{1,2}]], dotRows[[All,{1,3}]], dotRows[[All,{1,4}]]},
    Sequence @@ lineOpts["dot(a, b)  —  \[Mu]s vs n",
      {Directive[nC,Thick], Directive[wC,Thick], Directive[xC,Thick]}]],
  mkLegend[{"native","wstp","wxf"}, {nC, wC, xC}]];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* scale_array                                                                *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["Benchmarking scale_array..."];
scRows = Table[
  Module[{as = mkNA[n], r = Max[1, Round[4000/n*100]]},
    idx = 0;
    {n,
     timeMicros[Function[Module[{j=nextI[]}, Total @ nativeScale[as[[j]], 2.]]], r],
     timeMicros[Function[Module[{j=nextI[]}, Total @ Normal @ wstpScale[as[[j]], 2.]]], r],
     timeMicros[Function[Module[{j=nextI[]}, Total @ Normal @ wxfScale[as[[j]], 2.]]], r]}],
  {n, ns}];
Print @ Legended[
  ListLinePlot[{scRows[[All,{1,2}]], scRows[[All,{1,3}]], scRows[[All,{1,4}]]},
    Sequence @@ lineOpts["scale_array(arr, f)  —  \[Mu]s vs n",
      {Directive[nC,Thick], Directive[wC,Thick], Directive[xC,Thick]}]],
  mkLegend[{"native","wstp","wxf"}, {nC, wC, xC}]];

(* ══════════════════════════════════════════════════════════════════════════ *)
(* echo_dataset                                                               *)
(* ══════════════════════════════════════════════════════════════════════════ *)
Print["Benchmarking echo_dataset..."];
dsRows = Table[
  Module[{r = Max[1, Round[4000/n*100]],
          ds = <|"name" -> "t",
                 "values"  -> NumericArray[RandomReal[1, n], "Real64"],
                 "weights" -> NumericArray[RandomReal[1, n], "Real64"]|>},
    {n, timeMicros[Function[wxfDs[ds]], r]}],
  {n, ns}];
Print @ Legended[
  ListLinePlot[{dsRows},
    Sequence @@ lineOpts["echo_dataset(ds)  —  \[Mu]s vs n",
      {Directive[xC, Thick]}]],
  mkLegend[{"wxf"}, {xC}]];

Print["Done."];
