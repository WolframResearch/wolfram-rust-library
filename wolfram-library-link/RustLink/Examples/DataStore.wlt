Needs["MUnit`"]

VerificationTest[
    LibraryFunctionLoad[
        "libdata_store",
        "string_join",
        {"DataStore"},
        String
    ][
        Developer`DataStore["hello", " ", "world"]
    ],
    "hello world",
    TestID -> "RustLink-DataStore"
]