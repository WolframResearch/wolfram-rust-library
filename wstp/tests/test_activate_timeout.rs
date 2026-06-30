//! Tests for `Link::activate_with_timeout` and
//! `WolframKernelProcess::launch_with_timeout`.
//!
//! The kernel-launching test in this file is gated on the
//! `WSTP_RUN_KERNEL_TESTS` environment variable, because launching a Wolfram
//! Kernel requires a local Wolfram installation. Set
//! `WSTP_RUN_KERNEL_TESTS=1` to opt in.
//!
//! The two pure-WSTP tests (no kernel needed) run unconditionally.

use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use wstp::{Link, Protocol};

#[test]
fn activate_with_timeout_returns_err_when_no_peer_connects() {
    let mut listener = Link::listen(Protocol::SharedMemory, "")
        .expect("listen() should succeed for a fresh SharedMemory link");

    let start = Instant::now();
    let result = listener.activate_with_timeout(Duration::from_millis(500));
    let elapsed = start.elapsed();

    assert!(
        result.is_err(),
        "expected activate_with_timeout to error when no peer connects, got {result:?}"
    );

    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("timed out") || msg.to_lowercase().contains("timeout"),
        "expected timeout-flavoured error message, got: {msg}"
    );

    assert!(
        elapsed >= Duration::from_millis(500),
        "activate_with_timeout returned too early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "activate_with_timeout took too long to abort: {elapsed:?}"
    );
}

#[test]
fn activate_with_timeout_succeeds_when_peer_connects_within_window() {
    let mut listener = Link::listen(Protocol::SharedMemory, "")
        .expect("listen() should succeed");
    let name = listener.link_name();
    assert!(!name.is_empty());

    let listener_thread = thread::spawn(move || {
        listener
            .activate_with_timeout(Duration::from_secs(5))
            .expect("listener-side activate_with_timeout should succeed");
        listener
    });

    let connecter_thread = thread::spawn(move || {
        // Give the listener a moment to be ready.
        thread::sleep(Duration::from_millis(100));
        let mut connecter = Link::connect(Protocol::SharedMemory, &name)
            .expect("connect() should succeed");
        connecter
            .activate_with_timeout(Duration::from_secs(5))
            .expect("connecter-side activate_with_timeout should succeed");
        connecter
    });

    let _listener = listener_thread.join().expect("listener thread panicked");
    let _connecter = connecter_thread.join().expect("connecter thread panicked");
}

/// Verify that a second activation attempt after a timeout still behaves
/// sensibly (the yield function is properly restored, the link state is not
/// corrupted, and the deadline thread-local is cleared).
#[test]
fn activate_with_timeout_can_be_called_twice_on_same_thread() {
    let mut a = Link::listen(Protocol::SharedMemory, "")
        .expect("listen() should succeed");
    let _ = a.activate_with_timeout(Duration::from_millis(200));

    let mut b = Link::listen(Protocol::SharedMemory, "")
        .expect("listen() should succeed");
    let start = Instant::now();
    let result = b.activate_with_timeout(Duration::from_millis(200));
    let elapsed = start.elapsed();

    assert!(result.is_err(), "expected second timeout to also error");
    assert!(
        elapsed >= Duration::from_millis(200),
        "second activate_with_timeout returned too early: {elapsed:?}"
    );
    assert!(
        elapsed < Duration::from_secs(2),
        "second activate_with_timeout took too long: {elapsed:?}"
    );
}

/// Verify that `launch_with_timeout` kills the spawned process when activation
/// times out, rather than leaving it as an orphan.
///
/// We spawn a shell script that writes its own PID to a temp file and then
/// sleeps; after the timeout we confirm the PID is no longer alive.
#[cfg(unix)]
#[test]
fn launch_with_timeout_kills_spawned_process_on_timeout() {
    use std::{fs, os::unix::fs::PermissionsExt};
    use wstp::kernel::WolframKernelProcess;

    let tmp = std::env::temp_dir();
    let script = tmp.join("wstp_test_fake_kernel.sh");
    let pid_file = tmp.join("wstp_test_pid.txt");

    // Write a long-lived fake "kernel" that records its own PID so we can
    // verify it was killed after the timeout.
    fs::write(
        &script,
        format!("#!/bin/sh\necho $$ > {}\nsleep 60\n", pid_file.display()),
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let _ = fs::remove_file(&pid_file);

    let result = WolframKernelProcess::launch_with_timeout(&script, Duration::from_millis(400));
    assert!(result.is_err(), "expected timeout error, got {result:?}");

    // The pid_file may not exist if the script never got to run `echo $$`
    // (e.g. the OS was very slow to start it).  That's an acceptable skip.
    if let Ok(pid_str) = fs::read_to_string(&pid_file) {
        let pid: u32 = pid_str.trim().parse().expect("pid file should contain a number");
        // `kill -0 <pid>` exits 0 if the process exists, non-zero otherwise.
        let alive = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(!alive, "spawned process (PID {pid}) should have been killed on timeout");
    }

    let _ = fs::remove_file(&script);
    let _ = fs::remove_file(&pid_file);
}

/// Kernel test: when given a path that does not exist (or refuses to start),
/// `launch_with_timeout` should error within roughly `timeout`, instead of
/// hanging forever like `launch()` would under the same scenario.
///
/// Gated on `WSTP_RUN_KERNEL_TESTS=1` to keep CI configurations that don't
/// have a Wolfram installation happy, even though this particular test does
/// not actually require a working kernel.
#[test]
fn launch_with_timeout_errors_on_missing_kernel_within_window() {
    if std::env::var("WSTP_RUN_KERNEL_TESTS").ok().as_deref() != Some("1") {
        eprintln!(
            "skipping launch_with_timeout_errors_on_missing_kernel_within_window: \
             set WSTP_RUN_KERNEL_TESTS=1 to enable"
        );
        return;
    }

    use wstp::kernel::WolframKernelProcess;

    // A path that exists but is not a Wolfram Kernel: /bin/sleep on Unix,
    // which will be spawned but never speak WSTP back to us.
    let fake_kernel: PathBuf = if cfg!(unix) {
        PathBuf::from("/bin/sleep")
    } else {
        // On Windows, ping.exe is universally present and runs long enough
        // to never satisfy the WSTP handshake.
        PathBuf::from("ping")
    };

    let start = Instant::now();
    let result = WolframKernelProcess::launch_with_timeout(
        &fake_kernel,
        Duration::from_secs(1),
    );
    let elapsed = start.elapsed();

    assert!(
        result.is_err(),
        "expected launch_with_timeout to error for non-kernel binary"
    );
    assert!(
        elapsed < Duration::from_millis(1500),
        "launch_with_timeout took too long: {elapsed:?}"
    );
}
