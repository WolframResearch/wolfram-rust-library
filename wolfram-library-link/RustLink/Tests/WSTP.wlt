Needs["MUnit`"]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_wstp_fn_empty",
		LinkObject,
		LinkObject
	][],
	(* The empty arguments list is never read, so it's left on the link and assumed to be
	   the return value. *)
	{},
	SameTest -> MatchQ
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_wstp_fn_panic_immediately",
		LinkObject,
		LinkObject
	][],
	Failure["RustPanic", <|
		"Message" -> "successful panic",
		(* Avoid hard-coding the panic line/column number into the test. *)
		"SourceLocation" -> s_?StringQ /; StringStartsQ[s, "wolfram-library-link/examples/tests/test_wstp.rs:"],
		"Backtrace" -> Missing["NotEnabled"]
	|>],
	SameTest -> MatchQ
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_wstp_fn_panic_immediately_with_formatting",
		LinkObject,
		LinkObject
	][],
	Failure["RustPanic", <|
		"Message" -> "successful formatted panic",
		(* Avoid hard-coding the panic line/column number into the test. *)
		"SourceLocation" -> s_?StringQ /; StringStartsQ[s, "wolfram-library-link/examples/tests/test_wstp.rs:"],
		"Backtrace" -> Missing["NotEnabled"]
	|>],
	SameTest -> MatchQ
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_wstp_fn_panic_partial_result",
		LinkObject,
		LinkObject
	][],
	Failure["RustPanic", <|
		"Message" -> "incomplete result",
		(* Avoid hard-coding the panic line/column number into the test. *)
		"SourceLocation" -> s_?StringQ /; StringStartsQ[s, "wolfram-library-link/examples/tests/test_wstp.rs:"],
		"Backtrace" -> Missing["NotEnabled"]
	|>],
	SameTest -> MatchQ
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_wstp_fn_return_partial_result",
		LinkObject,
		LinkObject
	][],
	Unevaluated @ LibraryFunction[
		s_String /; StringEndsQ[
			s,
			RepeatedNull["lib", 1]
			~~ "library_tests."
			~~ ("dylib" | "dll" | "so")
		],
		"test_wstp_fn_return_partial_result",
		LinkObject
	][],
	SameTest -> MatchQ
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_wstp_fn_poison_link_and_panic",
		LinkObject,
		LinkObject
	][],
	Failure["RustPanic", <|
		"Message" -> "successful panic",
		(* Avoid hard-coding the panic line/column number into the test. *)
		"SourceLocation" -> s_?StringQ /; StringStartsQ[s, "wolfram-library-link/examples/tests/test_wstp.rs:"],
		"Backtrace" -> Missing["NotEnabled"]
	|>],
	SameTest -> MatchQ
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_wstp_panic_with_empty_link",
		LinkObject,
		LinkObject
	][],
	Failure["RustPanic", <|
		"Message" -> "panic while !link.is_ready()",
		"SourceLocation" -> s_?StringQ /; StringStartsQ[s, "wolfram-library-link/examples/tests/test_wstp.rs:"],
		"Backtrace" -> Missing["NotEnabled"]
	|>],
	SameTest -> MatchQ
]

(*====================================*)
(* Vec<Expr>                          *)
(*====================================*)

VerificationTest[
	Block[{$Context = "UnusedContext`", $ContextPath = {}},
		LibraryFunctionLoad[
			"liblibrary_tests",
			"test_wstp_expr_return_null",
			LinkObject,
			LinkObject
		][]
	],
	Null,
	SameTest -> MatchQ
]