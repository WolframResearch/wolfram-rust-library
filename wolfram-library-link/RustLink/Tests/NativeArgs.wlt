Needs["MUnit`"]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_no_args",
		{},
		Integer
	][],
	4
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_ret_void",
		{},
		"Void"
	][],
	Null
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_mint",
		{Integer},
		Integer
	][5],
	25
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_raw_mint",
		{Integer},
		Integer
	][9],
	81
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_mint_mint",
		{Integer, Integer},
		Integer
	][5, 10],
	15
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_mreal",
		{Real},
		Real
	][2.5],
	6.25
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_i64",
		{Integer},
		Integer
	][5],
	25
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_i64_i64",
		{Integer, Integer},
		Integer
	][5, 10],
	15
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_f64",
		{Real},
		Real
	][2.5],
	6.25
]

(*---------*)
(* Strings *)
(*---------*)

(* Test[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_str",
		{String},
		String
	]["hello"],
	"olleh"
] *)

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_string",
		{String},
		String
	]["hello"],
	"olleh"
]

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_c_string",
		{String},
		Integer
	]["hello world"],
	11
]

(*---------*)
(* Panics  *)
(*---------*)

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_panic",
		{},
		"Void"
	][],
	LibraryFunctionError["LIBRARY_USER_ERROR", 1002],
	{LibraryFunction::rterr}
]

(*----------------*)
(* NumericArray's *)
(*----------------*)

VerificationTest[
	totalI64 = LibraryFunctionLoad[
		"liblibrary_tests",
		"total_i64",
		{LibraryDataType[NumericArray, "Integer64"]},
		Integer
	];

	totalI64[NumericArray[Range[100], "Integer64"]],
	5050
]

VerificationTest[
	positiveQ = LibraryFunctionLoad[
		"liblibrary_tests",
		"positive_i64",
		{LibraryDataType[NumericArray, "Integer64"]},
		LibraryDataType[NumericArray, "UnsignedInteger8"]
	];

	positiveQ[NumericArray[{0, 1, -2, 3, 4,	-5}, "Integer64"]],
	NumericArray[{0, 1, 0, 1, 1, 0}, "UnsignedInteger8"]
]