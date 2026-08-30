(* TCP streams example end-to-end tests.
   Run via: cargo wl test  (from wolfram-examples/)

   These open a real TCP connection to a loopback echo server that the example
   starts for the purpose, write to it through an output stream, and read the
   echo back through an input stream:

     tcp::open_connection    -> a connection id
     OpenWrite[id, Method -> "TCP"] / OpenRead[id, Method -> "TCP"]
     tcp::close_connection   -> True if the connection was open

   The connection outlives the streams, so a request can be written and a
   response read over the same id. *)

$Libs = Quiet[Get["Functions.wl"]];

If[!AssociationQ[$Libs],
    Print["SKIP: could not load Functions.wl"]; Return[]
];
If[!KeyExistsQ[$Libs, "tcp::open_connection"],
    Print["SKIP: tcp::connect not found"]; Return[]
];

$Connect   = $Libs["tcp::open_connection"];
$Close     = $Libs["tcp::close_connection"];
$IsOpen    = $Libs["tcp::connection_is_open"];
$EchoPort  = $Libs["tcp::start_echo_server"];

$Port = $EchoPort[];

(* Write a string to a connection and read the echo back. *)
roundTrip[id_, text_] := Module[{out, in, echoed},
    out = OpenWrite[ToString[id], Method -> "TCP"];
    WriteString[out, text];
    Close[out];

    (* Read exactly as many bytes as were written: the peer holds the
       connection open, so there is no end of stream to read up to. *)
    in = OpenRead[ToString[id], Method -> "TCP", BinaryFormat -> True];
    echoed = FromCharacterCode[BinaryReadList[in, "Byte", StringLength[text]]];
    Close[in];

    echoed
];

(*====================================*)

VerificationTest[
    IntegerQ[$Port] && $Port > 0
    ,
    True
    ,
    TestID -> "TcpStreams-echo-server-starts"
]

VerificationTest[
    id = $Connect["127.0.0.1", $Port];
    result = IntegerQ[id] && $IsOpen[id];
    $Close[id];
    result
    ,
    True
    ,
    TestID -> "TcpStreams-connect-and-close"
]

VerificationTest[
    id = $Connect["127.0.0.1", $Port];
    $Close[id];
    $IsOpen[id]
    ,
    False
    ,
    TestID -> "TcpStreams-closed-connection-is-not-open"
]

(* Closing a connection that was never open reports that, rather than failing. *)
VerificationTest[
    $Close[999999]
    ,
    False
    ,
    TestID -> "TcpStreams-close-unknown-connection"
]

(*====================================*)
(* Streams over a connection          *)
(*====================================*)

VerificationTest[
    id = $Connect["127.0.0.1", $Port];
    echoed = roundTrip[id, "hello over TCP"];
    $Close[id];
    echoed
    ,
    "hello over TCP"
    ,
    TestID -> "TcpStreams-write-then-read-echo"
]

(* The connection outlives its streams: a second round trip reuses the same id. *)
VerificationTest[
    id = $Connect["127.0.0.1", $Port];
    first = roundTrip[id, "first"];
    second = roundTrip[id, "second"];
    $Close[id];
    {first, second}
    ,
    {"first", "second"}
    ,
    TestID -> "TcpStreams-connection-outlives-streams"
]

VerificationTest[
    id = $Connect["127.0.0.1", $Port];
    out = OpenWrite[ToString[id], Method -> "TCP"];
    WriteString[out, "0123456789"];
    Close[out];

    in = OpenRead[ToString[id], Method -> "TCP", BinaryFormat -> True];
    bytes = BinaryReadList[in, "Byte", 10];
    Close[in];
    $Close[id];

    bytes
    ,
    ToCharacterCode["0123456789"]
    ,
    TestID -> "TcpStreams-binary-read"
]

(*====================================*)
(* Errors                             *)
(*====================================*)

(* Opening a stream over an id that is not open fails, with the reason. *)
VerificationTest[
    OpenRead["999999", Method -> "TCP"]
    ,
    $Failed
    ,
    {OpenRead::noopen}
    ,
    TestID -> "TcpStreams-open-stream-on-unknown-connection"
]

(* A stream name that is not an id at all fails the same way. *)
VerificationTest[
    OpenRead["not-a-connection-id", Method -> "TCP"]
    ,
    $Failed
    ,
    {OpenRead::noopen}
    ,
    TestID -> "TcpStreams-open-stream-with-bad-name"
]
