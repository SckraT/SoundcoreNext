//! One thread for every test that creates a Slint component.
//!
//! Slint binds its platform to the thread that creates the first component and
//! refuses components made on another thread. The test harness runs each test
//! on its own thread, so tests cannot each open their own window: every one of
//! them runs its body on this shared thread instead, and a panic in the body is
//! raised again on the test's own thread so the test still fails normally.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{OnceLock, mpsc};

use crate::AppWindow;

/// What a job reports to the test that queued it: no panic, or the panic's
/// payload to raise again there.
type JobResult = Result<(), Box<dyn Any + Send>>;

/// A test body to run on the Slint thread, and the channel that reports back.
type Job = (Box<dyn FnOnce() + Send>, mpsc::Sender<JobResult>);

/// The queue of jobs for the one thread that creates Slint components.
fn jobs() -> &'static mpsc::Sender<Job> {
    static JOBS: OnceLock<mpsc::Sender<Job>> = OnceLock::new();
    JOBS.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("slint-test-ui".into())
            .spawn(move || {
                while let Ok((job, reply)) = receiver.recv() {
                    let result = catch_unwind(AssertUnwindSafe(job));
                    let _ = reply.send(result);
                }
            })
            .expect("the Slint test thread starts");
        sender
    })
}

/// Runs `body` on the Slint test thread and raises its panic on the caller's.
fn run(body: impl FnOnce() + Send + 'static) {
    let (reply, result) = mpsc::channel();
    jobs()
        .send((Box::new(body), reply))
        .expect("the Slint test thread takes the job");
    match result.recv().expect("the Slint test thread answers") {
        Ok(()) => {}
        Err(payload) => resume_unwind(payload),
    }
}

/// Runs `body` with a fresh shell window on the Slint test thread; the window is
/// dropped when `body` returns.
pub(crate) fn with_ui_window(body: impl FnOnce(&AppWindow) + Send + 'static) {
    run(move || {
        let ui = AppWindow::new().expect("the shell window builds");
        body(&ui);
    });
}

/// Runs `body` on the Slint test thread without a window, for the tests that
/// only create the tray component.
pub(crate) fn with_slint_thread(body: impl FnOnce() + Send + 'static) {
    run(body);
}
