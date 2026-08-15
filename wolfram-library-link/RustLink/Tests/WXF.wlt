Needs["MUnit`"]

wrapWXFFunction = Composition[BinaryDeserialize, #, BinarySerialize, List] &

VerificationTest[
	(wrapWXFFunction@LibraryFunctionLoad[
		"liblibrary_tests",
		"test_wxf_bool_input",
		{LibraryDataType[ByteArray]},
		LibraryDataType[ByteArray]]
	) /@ {True, False},
	{42, 24}
]

VerificationTest[
	(wrapWXFFunction@LibraryFunctionLoad[
		"liblibrary_tests",
		"test_wxf_bool_output",
		{LibraryDataType[ByteArray]},
		LibraryDataType[ByteArray]]
	) /@ {42, -42},
	{True, False}
]