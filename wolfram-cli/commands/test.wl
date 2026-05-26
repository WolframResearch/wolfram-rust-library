Function @ Module[{passed = 0, failed = 0},
  $LibraryPath = Join[#LibPaths, $LibraryPath];
  Module[{files = If[Length[#Files] === 0,
      FileNames["*.wlt", #Cwd, Infinity],
      FileNames[#Files, #Cwd]
    ]},
  Print["running from: ", #Cwd];
  Print["files: ", files];
  TestReport[
    files,
    HandlerFunctions -> <|
    "FileStarted" -> Function[{ev},
      Print["testing ", FileNameTake[ev["TestFileName"]]]
    ],
    "TestEvaluated" -> Function[{ev},
      Module[{obj = First[ev["TestObject"]], outcome = ev["Outcome"]},
        If[outcome === "Success",
          passed++;
          Print["ok   ", obj["TestID"]],
          failed++;
          Print["FAIL ", obj["TestID"]];
          Print["     input:    ", ToString[obj["Input"], OutputForm]];
          Print["     expected: ", ToString[obj["ExpectedOutput"], OutputForm]];
          Print["     got:      ", ToString[obj["ActualOutput"], OutputForm]]
        ]
      ]
    ],
    "ReportCompleted" -> Function[{ev}, Null]
  |>];
  StringJoin[
    "test result: ", If[failed == 0, "ok", "FAILED"], ". ",
    ToString[passed], " passed; ", ToString[failed], " failed"
  ]
]]
