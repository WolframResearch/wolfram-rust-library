VerificationTest[
    LibraryLoad["libwll_docs"];

    stream = OpenRead["hello", Method -> "RustEcho"];
    contents = ReadString[stream];
    Close[stream];

    contents
    ,
    "you opened: hello"
]
