
Needs["MUnit`"]

(*
	Tests for `wolfram_library_link::stream`. The Rust side of these tests is
	`wolfram-library-link/examples/tests/test_streams.rs`.

	Several tests assert on what the *library* observed, via the
	`test_stream_*` functions, because the Wolfram Language does not otherwise
	show what a stream method was handed.
*)

$reset = LibraryFunctionLoad["liblibrary_tests", "test_stream_reset", {}, "Boolean"];
$lastOptions = LibraryFunctionLoad["liblibrary_tests", "test_stream_last_options", {}, String];
$lastOpenRequest = LibraryFunctionLoad["liblibrary_tests", "test_stream_last_open_request", {}, String];
$takeWritten = LibraryFunctionLoad["liblibrary_tests", "test_stream_take_written", {}, String];
$waitCount = LibraryFunctionLoad["liblibrary_tests", "test_stream_wait_count", {}, Integer];
$outputMode = LibraryFunctionLoad["liblibrary_tests", "test_stream_output_mode", {}, String];
$duplicateFails = LibraryFunctionLoad["liblibrary_tests", "test_stream_duplicate_registration_panics", {}, "Boolean"];

(*====================================*)
(* Reading                            *)
(*====================================*)

VerificationTest[
	$reset[];
	stream = OpenRead["anything", Method -> "TestFixed"];
	contents = ReadString[stream];
	Close[stream];
	contents
	,
	"hello from Rust"
	,
	TestID -> "Streams-read-fixed-contents"
]

VerificationTest[
	stream = OpenRead["anything", Method -> "TestFixed", BinaryFormat -> True];
	bytes = BinaryReadList[stream, "Byte"];
	Close[stream];
	bytes
	,
	ToCharacterCode["hello from Rust"]
	,
	TestID -> "Streams-read-binary"
]

(* A stream that reports `Ok(0)` from `read` is at end of stream. *)
VerificationTest[
	stream = OpenRead["anything", Method -> "TestFixed"];
	ReadString[stream];
	second = ReadString[stream];
	Close[stream];
	second
	,
	EndOfFile
	,
	TestID -> "Streams-read-past-end-is-EndOfFile"
]

(*====================================*)
(* OpenRequest                        *)
(*====================================*)

VerificationTest[
	$reset[];
	stream = OpenRead["some-stream-name", Method -> "TestFixed"];
	Close[stream];
	StringContainsQ[$lastOpenRequest[], "name: \"some-stream-name\""]
	,
	True
	,
	TestID -> "Streams-open-request-name"
]

(* `msgHead` is a fully qualified symbol name, not a short name. *)
VerificationTest[
	$reset[];
	stream = OpenRead["anything", Method -> "TestFixed"];
	Close[stream];
	StringContainsQ[$lastOpenRequest[], "System`OpenRead"]
	,
	True
	,
	TestID -> "Streams-open-request-message-head"
]

(*====================================*)
(* StreamOptions                      *)
(*====================================*)

(*
	The "TestOptions" method reads the options link and serves the resulting
	expression back as the stream's contents, so these tests check the whole
	`StreamOptions` -> `wstp::Link` -> `Expr` path.
*)

VerificationTest[
	$reset[];
	stream = OpenRead["anything", Method -> "TestOptions"];
	contents = ReadString[stream];
	Close[stream];
	{StringQ[contents], StringContainsQ[contents, "<error reading options"],
		StringContainsQ[contents, "<no options link"]}
	,
	{True, False, False}
	,
	TestID -> "Streams-options-link-is-readable"
]

(* The options expression must round-trip back into a real expression. *)
VerificationTest[
	$reset[];
	stream = OpenRead["anything", Method -> "TestOptions"];
	contents = ReadString[stream];
	Close[stream];
	Head[ToExpression[contents]] =!= $Failed
	,
	True
	,
	TestID -> "Streams-options-expression-parses"
]

(* An option passed at `OpenRead` reaches the library. *)
VerificationTest[
	$reset[];
	stream = OpenRead["anything", Method -> "TestOptions", BinaryFormat -> True];
	contents = ReadString[stream];
	Close[stream];
	StringContainsQ[contents, "BinaryFormat"]
	,
	True
	,
	TestID -> "Streams-options-include-open-options"
]

(*====================================*)
(* Errors                             *)
(*====================================*)

(*
	A `StreamError` becomes `General::strmerr` under the reading function's own
	tag, with the message inserted verbatim.
*)
VerificationTest[
	$reset[];
	stream = OpenRead["anything", Method -> "TestError"];
	contents = ReadString[stream];
	Close[stream];
	contents
	,
	EndOfFile
	,
	{Read::strmerr}
	,
	TestID -> "Streams-read-error-issues-strmerr"
]

(*
	The message text is the `StreamError`'s message, inserted verbatim. Capture
	the rendered message by pointing `$Messages` at one of our own output
	streams.
*)
VerificationTest[
	$reset[];
	stream = OpenRead["anything", Method -> "TestError"];
	sink = OpenWrite["message-sink", Method -> "TestCollect"];
	Block[{$Messages = {sink}}, ReadString[stream]];
	Close[sink];
	Close[stream];
	StringContainsQ[$takeWritten[], "The test stream failed on purpose."]
	,
	True
	,
	{Read::strmerr}
	,
	TestID -> "Streams-read-error-text-is-verbatim"
]

(* A panic's message reaches the user the same way. *)
VerificationTest[
	$reset[];
	stream = OpenRead["anything", Method -> "TestPanic"];
	sink = OpenWrite["message-sink", Method -> "TestCollect"];
	Block[{$Messages = {sink}}, ReadString[stream]];
	Close[sink];
	Close[stream];
	StringContainsQ[$takeWritten[], "The test stream panicked on purpose."]
	,
	True
	,
	{Read::strmerr}
	,
	TestID -> "Streams-panic-message-reaches-user"
]

(* A method whose `open` fails makes `OpenRead` fail. *)
VerificationTest[
	OpenRead["anything", Method -> "TestOpenFail"]
	,
	$Failed
	,
	{OpenRead::noopen}
	,
	TestID -> "Streams-open-failure"
]

(* A panic inside a stream callback is contained, not propagated into the kernel. *)
VerificationTest[
	$reset[];
	stream = OpenRead["anything", Method -> "TestPanic"];
	contents = ReadString[stream];
	Close[stream];
	contents
	,
	EndOfFile
	,
	{Read::strmerr}
	,
	TestID -> "Streams-panic-is-contained"
]

(*====================================*)
(* Waiting for input                  *)
(*====================================*)

(*
	`StreamError::WouldBlock` must not read as end of stream: the kernel should
	wait and retry until real data arrives.
*)
VerificationTest[
	$reset[];
	stream = OpenRead["anything", Method -> "TestBlocking"];
	contents = ReadString[stream];
	Close[stream];
	contents
	,
	"unblocked"
	,
	TestID -> "Streams-would-block-is-retried"
]

VerificationTest[
	$reset[];
	stream = OpenRead["anything", Method -> "TestBlocking"];
	ReadString[stream];
	Close[stream];
	$waitCount[] > 0
	,
	True
	,
	TestID -> "Streams-would-block-calls-wait-for-input"
]

(*====================================*)
(* Seeking                            *)
(*====================================*)

VerificationTest[
	stream = OpenRead["anything", Method -> "TestSeekable", BinaryFormat -> True];
	position = StreamPosition[stream];
	Close[stream];
	position
	,
	0
	,
	TestID -> "Streams-seekable-initial-position"
]

(*
	Seek far enough that the target is outside the kernel's read buffer, so this
	actually reaches `InputStream::seek` rather than being served from the
	kernel's own buffer. Byte `i` of the test stream is `Mod[i, 251]`.
*)
VerificationTest[
	stream = OpenRead["anything", Method -> "TestSeekable", BinaryFormat -> True];
	SetStreamPosition[stream, 200000];
	bytes = BinaryReadList[stream, "Byte", 4];
	Close[stream];
	bytes
	,
	Mod[Range[200000, 200003], 251]
	,
	TestID -> "Streams-seek-outside-buffer"
]

VerificationTest[
	stream = OpenRead["anything", Method -> "TestSeekable", BinaryFormat -> True];
	SetStreamPosition[stream, 200000];
	BinaryReadList[stream, "Byte", 4];
	position = StreamPosition[stream];
	Close[stream];
	position
	,
	200004
	,
	TestID -> "Streams-seek-then-position"
]

(* Seeking backwards clears end-of-stream. *)
VerificationTest[
	stream = OpenRead["anything", Method -> "TestSeekable", BinaryFormat -> True];
	SetStreamPosition[stream, 299998];
	BinaryReadList[stream, "Byte"];
	SetStreamPosition[stream, 0];
	bytes = BinaryReadList[stream, "Byte", 3];
	Close[stream];
	bytes
	,
	{0, 1, 2}
	,
	TestID -> "Streams-seek-clears-end-of-stream"
]

(* An open stream reports the method it was opened with. *)
VerificationTest[
	stream = OpenRead["anything", Method -> "TestFixed"];
	options = Options[stream];
	Close[stream];
	MemberQ[options, HoldPattern[Method -> _]]
	,
	True
	,
	TestID -> "Streams-options-of-open-stream-report-method"
]

(*====================================*)
(* Name dispatch                      *)
(*====================================*)

(*
	`NAME_DISPATCH = true` lets a method claim names without an explicit
	`Method` option.
*)
VerificationTest[
	stream = OpenRead["teststream://something"];
	contents = ReadString[stream];
	Close[stream];
	contents
	,
	"claimed by name"
	,
	TestID -> "Streams-name-dispatch-claims-matching-name"
]

VerificationTest[
	OpenRead["definitely-not-a-test-stream-name"]
	,
	$Failed
	,
	{OpenRead::noopen}
	,
	TestID -> "Streams-name-dispatch-declines-other-names"
]

(*====================================*)
(* Writing                            *)
(*====================================*)

VerificationTest[
	$reset[];
	stream = OpenWrite["anything", Method -> "TestCollect"];
	WriteString[stream, "written from WL"];
	Close[stream];
	$takeWritten[]
	,
	"written from WL"
	,
	TestID -> "Streams-write-collects-bytes"
]

VerificationTest[
	$reset[];
	stream = OpenWrite["anything", Method -> "TestCollect"];
	WriteString[stream, "abc"];
	position = StreamPosition[stream];
	Close[stream];
	position
	,
	3
	,
	TestID -> "Streams-output-stream-position"
]

VerificationTest[
	$reset[];
	stream = OpenWrite["anything", Method -> "TestCollect"];
	Close[stream];
	$outputMode[]
	,
	"Truncate"
	,
	TestID -> "Streams-open-write-is-truncate-mode"
]

VerificationTest[
	$reset[];
	stream = OpenAppend["anything", Method -> "TestCollect"];
	Close[stream];
	$outputMode[]
	,
	"Append"
	,
	TestID -> "Streams-open-append-is-append-mode"
]

(*
	Nothing in the Wolfram Language retries a partial write, so the wrapper
	loops until the whole buffer is consumed. "TestShortWrite" accepts one byte
	per call; every byte must still arrive.
*)
VerificationTest[
	$reset[];
	stream = OpenWrite["anything", Method -> "TestShortWrite"];
	WriteString[stream, "0123456789"];
	Close[stream];
	$takeWritten[]
	,
	"0123456789"
	,
	TestID -> "Streams-short-writes-are-retried"
]

(*====================================*)
(* Derived streams                    *)
(*====================================*)

(*
	`#[derive(InputStream)]`, `#[derive(SeekableInputStream)]` and
	`#[derive(OutputStream)]` forward to a type's own `std::io` impls. These
	check that the derived impls behave the same as the hand-written ones.
*)

VerificationTest[
	stream = OpenRead["anything", Method -> "TestDerivedRead"];
	contents = ReadString[stream];
	Close[stream];
	contents
	,
	"derived read"
	,
	TestID -> "Streams-derived-input-stream-reads"
]

(* `#[derive(SeekableInputStream)]` must also report the stream as seekable. *)
VerificationTest[
	stream = OpenRead["anything", Method -> "TestDerivedSeek", BinaryFormat -> True];
	SetStreamPosition[stream, 200000];
	bytes = BinaryReadList[stream, "Byte", 4];
	Close[stream];
	bytes
	,
	Mod[Range[200000, 200003], 251]
	,
	TestID -> "Streams-derived-seekable-stream-seeks"
]

VerificationTest[
	stream = OpenRead["anything", Method -> "TestDerivedSeek", BinaryFormat -> True];
	SetStreamPosition[stream, 200000];
	BinaryReadList[stream, "Byte", 4];
	position = StreamPosition[stream];
	Close[stream];
	position
	,
	200004
	,
	TestID -> "Streams-derived-seekable-stream-position"
]

VerificationTest[
	$reset[];
	stream = OpenWrite["anything", Method -> "TestDerivedWrite"];
	WriteString[stream, "derived write"];
	Close[stream];
	$takeWritten[]
	,
	"derived write"
	,
	TestID -> "Streams-derived-output-stream-writes"
]

(*====================================*)
(* Registration                       *)
(*====================================*)

(* Registering a name that is already taken panics rather than silently winning. *)
VerificationTest[
	$duplicateFails[]
	,
	True
	,
	TestID -> "Streams-duplicate-registration-panics"
]
