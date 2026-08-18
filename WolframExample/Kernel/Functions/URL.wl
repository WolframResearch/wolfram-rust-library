(* Wrappers for Libs/url — namespace "url".

   The Rust side is list-in / list-out and nothing else — one WXF round trip
   per call, not per string, which is where the win over the kernel's own
   URLEncode comes from on any real workload. The scalar form below is the
   whole of the single-string API: wrap, call, unwrap.

   A load failure comes back from libraryFunction[] as a Failure, which is not
   a one-element list, so the Replace leaves it alone and it propagates. *)

ExampleURLEncode[s_String] := Replace[ExampleURLEncode[{s}], {r_} :> r];
ExampleURLEncode[list : {___String}] := libraryFunction["url::url_encode"][list];

ExampleURLDecode[s_String] := Replace[ExampleURLDecode[{s}], {r_} :> r];
ExampleURLDecode[list : {___String}] := libraryFunction["url::url_decode"][list];
