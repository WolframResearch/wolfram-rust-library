//! Raise asynchronous events from a thread the library already owns.
//!
//! [`AsyncTaskObject::spawn_with_thread()`] asks the Wolfram Language to create
//! a thread to run the task on. That does not suit a library whose work is
//! already happening somewhere — on a worker thread it started earlier, in a
//! callback from a C library, or on an event loop — because the task would then
//! own a second thread with nothing to do.
//!
//! [`AsyncTaskObject::create_without_thread()`] creates the task on its own.
//! Whatever is already running raises the events, and nothing tears the task
//! down implicitly, so it has to be removed once the work it stands for is
//! over.
//!
//! See `RustLink/Examples/AsyncExamples.wlt` for example usage of this function.

use std::{thread, time::Duration};

use wolfram_library_link::{self as wll, sys::mint, AsyncTaskObject, DataStore};

/// Start an asynchronous task that raises `count` "tick" events, one every
/// `interval_ms` milliseconds, from a thread this library owns.
#[wll::export]
fn start_ticker(interval_ms: mint, count: mint) -> mint {
    let task = AsyncTaskObject::create_without_thread();
    // Read the id before the task moves onto the ticking thread; it is what the
    // Wolfram Language needs back from this function.
    let id = task.id();

    // Stands in for whatever a real library would already have running.
    thread::spawn(move || {
        for tick in 1..=count {
            thread::sleep(Duration::from_millis(interval_ms as u64));

            // The Wolfram Language side can stop the task at any point, after
            // which raising an event does nothing.
            if !task.is_alive() {
                break;
            }

            let mut data = DataStore::new();
            data.add_i64(tick);
            task.raise_async_event("tick", data);
        }

        // No thread's return marks this task finished, so say so explicitly.
        task.remove();
    });

    id
}
