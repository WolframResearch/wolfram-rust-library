
Needs["MUnit`"]

VerificationTest[
	func = LibraryFunctionLoad[
		"liblibrary_tests",
		"test_empty_data_store",
		{},
		"DataStore"
	];

	func[],
	Developer`DataStore[]
]

VerificationTest[
	func = LibraryFunctionLoad[
		"liblibrary_tests",
		"test_single_int_data_store",
		{},
		"DataStore"
	];

	func[],
	Developer`DataStore[1]
]

VerificationTest[
	func = LibraryFunctionLoad[
		"liblibrary_tests",
		"test_multiple_int_data_store",
		{},
		"DataStore"
	];

	func[],
	Developer`DataStore[1, 2, 3]
]

VerificationTest[
	func = LibraryFunctionLoad[
		"liblibrary_tests",
		"test_unnamed_heterogenous_data_store",
		{},
		"DataStore"
	];

	func[],
	Developer`DataStore[1, 2.0, "hello"]
]

VerificationTest[
	func = LibraryFunctionLoad[
		"liblibrary_tests",
		"test_named_heterogenous_data_store",
		{},
		"DataStore"
	];

	func[],
	Developer`DataStore[
		"an i64" -> 1,
		"an f64" -> 2.0,
		"a str" -> "hello"
	]
]

VerificationTest[
	func = LibraryFunctionLoad[
		"liblibrary_tests",
		"test_named_and_unnamed_heterogenous_data_store",
		{},
		"DataStore"
	];

	func[],
	Developer`DataStore[1, "real" -> 2.0, "hello" -> "world"]
]

(*====================================*)
(* Non-atomic types                   *)
(*====================================*)

VerificationTest[
	func = LibraryFunctionLoad[
		"liblibrary_tests",
		"test_named_numeric_array_data_store",
		{},
		"DataStore"
	];

	func[],
	Developer`DataStore[
		"array" -> NumericArray[{1, 2, 3}, "Integer64"]
	]
]

VerificationTest[
	func = LibraryFunctionLoad[
		"liblibrary_tests",
		"test_nested_data_store",
		{},
		"DataStore"
	];

	func[],
	Developer`DataStore[
		"is_inner" -> False,
		Developer`DataStore["is_inner" -> True]
	]
]

VerificationTest[
	func = LibraryFunctionLoad[
		"liblibrary_tests",
		"test_iterated_nested_data_store",
		{},
		"DataStore"
	];

	func[],
	Developer`DataStore[
		Developer`DataStore[
			Developer`DataStore[
				Developer`DataStore["level" -> 0],
				"level" -> 1
			],
			"level" -> 2
		]
	]
]

(*====================================*)
(* DataStore arguments                *)
(*====================================*)

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_data_store_arg",
		{"DataStore"},
		Integer
	][
		Developer`DataStore["a", "b", "c"]
	],
	3
]

(*====================================*)
(* DataStore nodes                    *)
(*====================================*)

VerificationTest[
	LibraryFunctionLoad[
		"liblibrary_tests",
		"test_data_store_nodes",
		{},
		"Void"
	][],
	Null
]