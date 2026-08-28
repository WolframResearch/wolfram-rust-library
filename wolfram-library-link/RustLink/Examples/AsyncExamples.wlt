Needs["MUnit`"]

(* Test the async_file_watcher_raw.rs example. *)
VerificationTest[
    delay = 100;
    file = CreateFile[];

    $changes = {};

    eventHandler[
        taskObject_,
        "change",
        {modTime_}
    ] := AppendTo[$changes, modTime];

    (* Begin the background task. *)
    task = Internal`CreateAsynchronousTask[
        LibraryFunctionLoad[
            "libasync_file_watcher_raw",
            "start_file_watcher",
            {Integer, "UTF8String"},
            Integer
        ],
        {delay, file},
        eventHandler
    ];

    (* Modify the watched file. *)
    Put[1, file];
    expectedModifiedTime = UnixTime[];

    (* Ensure the file modification check has time to run. *)
    Pause[Quantity[2 * delay, "Milliseconds"]];

    StopAsynchronousTask[task];

    $changes,
    {expectedModifiedTime},
    TestID -> "RustLink-AsyncExamples-1"
]

(* Test the async_file_watcher.rs example. This is identical to the above test, except the
   example implementation uses the safe wrappers. *)
VerificationTest[
    delay = 100;
    file = CreateFile[];

    $changes2 = {};

    eventHandler[
        taskObject_,
        "change",
        {modTime_}
    ] := AppendTo[$changes2, modTime];

    (* Begin the background task. *)
    task = Internal`CreateAsynchronousTask[
        LibraryFunctionLoad[
            "libasync_file_watcher",
            "start_file_watcher",
            {Integer, "UTF8String"},
            Integer
        ],
        {delay, file},
        eventHandler
    ];

    (* Modify the watched file. *)
    Put[1, file];
    expectedModifiedTime = UnixTime[];

    (* Ensure the file modification check has time to run. *)
    Pause[Quantity[2 * delay, "Milliseconds"]];

    StopAsynchronousTask[task];

    $changes2,
    {expectedModifiedTime},
    TestID -> "RustLink-AsyncExamples-2"
]

(* Test the async_ticker.rs example, which uses an async task that has no thread
   of its own: the events come from a thread the library started itself. *)
VerificationTest[
    interval = 50;
    count = 4;

    $ticks = {};

    tickHandler[taskObject_, "tick", {tick_}] := AppendTo[$ticks, tick];

    task = Internal`CreateAsynchronousTask[
        LibraryFunctionLoad[
            "libasync_ticker",
            "start_ticker",
            {Integer, Integer},
            Integer
        ],
        {interval, count},
        tickHandler
    ];

    (* Give every tick time to arrive, and the task time to remove itself. *)
    Pause[Quantity[4 * interval * count, "Milliseconds"]];

    (* The library removes the task once it has ticked the requested number of
       times, so it should no longer be listed as running. *)
    {$ticks, MemberQ[AsynchronousTasks[], task]},
    {Range[count], False},
    TestID -> "RustLink-AsyncExamples-3"
]
