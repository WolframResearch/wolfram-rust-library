(* ::Package:: *)

PacletObject[<|
    "Name" -> "WolframExample",
    "Version" -> "1.0.0",
    "WolframVersion" -> "14.1+",
    "Description" -> "Top-level Wolfram Language API over the Rust libraries packaged as the WolframExampleLib paclet.",
    "Creator" -> "Wolfram Research",
    "License" -> "MIT",
    (* Not a hard dependency: WolframExampleLib is built from Libs/ by
       `cargo wl build` and lives next to this paclet, so it is discovered
       through PacletDirectoryLoad rather than installed. Kernel/WolframExample.wl
       reports a readable message when it is missing. *)
    "Extensions" -> {
        {"Kernel",
            "Root" -> "Kernel",
            "Context" -> {"WolframExample`"}
        }
    }
|>]
