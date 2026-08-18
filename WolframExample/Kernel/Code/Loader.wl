(* Loading the generated library paclet. Get-ed by ../WolframExample.wl inside
   BeginPackage["WolframExample`"] / Begin["`Private`"], so everything defined
   here is private to the package.

   PacletObject[name]["AssetLocation", "Functions"] resolves the "Functions"
   asset that the generated PacletInfo.wl declares, so we never hard-code a
   path or a SystemID directory: the paclet manager picks the build matching
   this machine. (The property is singular — "AssetsLocation" is not one of
   PacletObject's properties and evaluates to an unevaluated part access.) *)

$libraryPaclet = "WolframExampleLib";

(* Everything that can go wrong here is reported as a Failure carrying the
   message template rather than as $Failed plus a Message: the object formats
   itself with the same text, survives being stored in a variable, and can be
   returned straight to the caller. *)
failure[tag_String, template_, params_List] := Failure[tag, <|
    "MessageTemplate" :> template,
    "MessageParameters" -> params
|>];

(* Each With sequence sees the previous one's bindings, so the three stages —
   find the paclet, resolve the asset, read it — chain without any mutable
   state; every stage guards on the one before it, and a single Which reports
   whichever one gave out. *)
loadFunctions[] := With[
    {paclet = PacletObject[$libraryPaclet]},
    {file = If[PacletObjectQ[paclet], paclet["AssetLocation", "Functions"], $Failed]},
    {functions = If[StringQ[file] && FileExistsQ[file], Get[file], $Failed]},
    Which[
        !PacletObjectQ[paclet],
            failure["MissingPaclet", WolframExample::nolib, {$libraryPaclet}],
        (* covers both an unusable asset path and a file that did not read
           back as an association: neither leaves `functions` one *)
        !AssociationQ[functions],
            failure["UnreadableAsset", WolframExample::badasset, {$libraryPaclet, file}],
        True,
            functions
    ]
];

(* Cache only a successful load, so a failed lookup can be retried after the
   library has been built or PacletDirectoryLoad-ed. *)
$functions := With[{loaded = loadFunctions[]},
    If[AssociationQ[loaded], $functions = loaded, loaded]
];

WolframExampleFunctions[] := $functions;

(* `libraryFunction["duckdb::db_query"][args]` — resolves lazily on each call.
   A library that failed to load, or a key that isn't in it (e.g. a Cargo
   feature was off when it was built), comes back as a Failure the caller can
   inspect — the load Failure is passed through unchanged rather than
   re-wrapped, so the original cause survives. *)
libraryFunction[key_String][args___] := With[
    {functions = $functions},
    {f = If[AssociationQ[functions], Lookup[functions, key, Missing[key]], functions]},
    Which[
        !AssociationQ[functions], functions,
        MissingQ[f], failure["UnknownFunction", WolframExample::nofunc, {key, $libraryPaclet}],
        True, f[args]
    ]
];
