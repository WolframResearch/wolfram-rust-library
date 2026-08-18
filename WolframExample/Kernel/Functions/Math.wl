(* Wrappers for Libs/math — namespace "math". *)

ExampleAdd[a_?NumericQ, b_?NumericQ] := libraryFunction["math::add"][N[a], N[b]];

(* The Rust side takes &NumericArray<f64>, so both arguments must be Real64
   NumericArrays; plain lists are converted here rather than in the library. *)
ExampleDot[a_List, b_List] := ExampleDot[toReal64[a], toReal64[b]];
ExampleDot[a_NumericArray, b_NumericArray] :=
    libraryFunction["math::dot"][a, b];

toReal64[list_List] := NumericArray[N[list], "Real64"];

(* One Rust function, two shapes: `area_shape` takes the `Shape` enum, whose
   WL form is a two-element {"VariantName", <|fields|>} — any head works, so
   "Circle" -> <|"radius" -> 1|> would do just as well. Struct fields keep
   their Rust names ("width", "radius", ...). *)
ExampleArea[assoc_?AssociationQ] := With[{shape = shapeFor[assoc]},
    If[FailureQ[shape], shape, libraryFunction["math::area_shape"][shape]]
];

shapeFor[assoc_] := Which[
    KeyExistsQ[assoc, "Radius"],
        {"Circle", <|"radius" -> N[assoc["Radius"]]|>},
    KeyExistsQ[assoc, "Width"] && KeyExistsQ[assoc, "Height"],
        {"Rect", <|"width" -> N[assoc["Width"]], "height" -> N[assoc["Height"]]|>},
    True,
        failure["UnknownShape", WolframExample::shape, {Keys[assoc]}]
];

ExampleSymmetricPoint[assoc_?AssociationQ] := With[
    {point = libraryFunction["math::symmetric_point"][
        <|"x" -> N[assoc["X"]], "y" -> N[assoc["Y"]]|>
    ]},
    If[AssociationQ[point], <|"X" -> point["x"], "Y" -> point["y"]|>, point]
];

(* A Rust `Result<f64, String>` crosses as the plain enum {"Ok", value} /
   {"Err", message} — only error types deriving `Failure` come back as a real
   Failure — so the wrapper is where that becomes idiomatic WL. *)
ExampleSafeDivide[a_?NumericQ, b_?NumericQ] := Replace[
    libraryFunction["math::safe_divide"][N[a], N[b]],
    {
        {"Ok", value_} :> value,
        {"Err", message_} :> failure["DivisionError", WolframExample::divide, {message}]
    }
];
