//! One copy of OpenSCL4 at a time, so a second start never fights the first for
//! the device or leaves a second icon in the notification area.
//!
//! The first start creates two named Windows objects for the app-data folder: a
//! mutex that says the app runs, and an auto-reset event that carries a "show
//! your window" request. A second start finds the mutex, sets the event and
//! ends; the first may have its window hidden in the notification area, so it
//! shows the window again. Once the claim is complete the handles are never
//! closed on purpose: the objects have to exist for as long as the process does,
//! and Windows releases them when it ends, so a crash leaves no stale lock
//! behind. If only one of the two objects can be created, the created handle is
//! closed again: a half-set-up start must not keep later starts thinking the app
//! is running.
//!
//! The event is waited on by a worker thread, which only sets an atomic flag;
//! the UI thread takes that flag on its existing worker-event tick and shows the
//! window itself, so no window handle ever crosses a thread boundary. No crate
//! is added: the app already depends on `windows`, and only its
//! `Win32_System_Threading` feature is switched on for these calls.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::{
    Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0},
    System::Threading::{CreateEventW, CreateMutexW, INFINITE, SetEvent, WaitForSingleObject},
};
use windows::core::HSTRING;

/// A stable name for an app-data folder: the folder path in lowercase, hashed
/// with FNV-1a. Windows folder names are case-insensitive, and std's hasher is
/// not guaranteed to give the same value in a later build, so the names of the
/// mutex and the event have to come from this instead.
fn folder_key(config_dir: &Path) -> String {
    let path = config_dir.to_string_lossy().to_lowercase();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in path.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The mutex and event names of an app-data folder. `Local\` keeps them inside
/// the logon session, like the shell's own per-user objects.
fn instance_names(config_dir: &Path) -> (String, String) {
    let key = folder_key(config_dir);
    (
        format!(r"Local\OpenSCL4-{key}"),
        format!(r"Local\OpenSCL4Show-{key}"),
    )
}

/// A second start's request for the running app to show its window.
#[derive(Debug, Clone)]
pub struct ShowRequest {
    requested: Arc<AtomicBool>,
}

impl ShowRequest {
    fn new() -> Self {
        Self {
            requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Takes the pending request, if a second start asked for the window. The
    /// UI thread calls this, so a request is never lost or seen twice.
    pub fn take_requested(&self) -> bool {
        self.requested.swap(false, Ordering::SeqCst)
    }

    /// Records one request from the watcher thread.
    fn request(&self) {
        self.requested.store(true, Ordering::SeqCst);
    }
}

/// The first start's claim on the app: hold it for as long as the app runs, so
/// the mutex object lives that long and a later start finds it.
pub struct FirstInstance {
    /// Kept open on purpose; see the module documentation.
    _mutex: HANDLE,
    show_request: ShowRequest,
}

impl FirstInstance {
    /// The requests of later starts to show this app's window.
    pub fn show_request(&self) -> ShowRequest {
        self.show_request.clone()
    }

    /// A claim for a start whose objects could not be created: the app still
    /// runs, but no later start can ask it to show its window.
    fn unclaimed() -> Self {
        Self {
            _mutex: HANDLE(std::ptr::null_mut()),
            show_request: ShowRequest::new(),
        }
    }
}

/// Claims the app for `config_dir`.
///
/// `Some` means this start runs the app; `None` means another start already
/// runs it, has been asked to show its window, and this start should end.
pub fn claim(config_dir: &Path) -> Option<FirstInstance> {
    let (mutex_name, event_name) = instance_names(config_dir);
    let mutex_name = HSTRING::from(mutex_name);
    let event_name = HSTRING::from(event_name);
    // SAFETY: plain calls with names that live through them. The handles stay
    // open for as long as the app runs, which is the point of the claim.
    let (mutex, already) = unsafe {
        let mutex = CreateMutexW(None, false, &mutex_name);
        (mutex, GetLastError() == ERROR_ALREADY_EXISTS)
    };
    // SAFETY: as above
    let event = unsafe { CreateEventW(None, false, false, &event_name) };
    let (mutex, event) = match (mutex, event) {
        (Ok(mutex), Ok(event)) => (mutex, event),
        (mutex, event) => {
            // A half-created claim is worse than none: a kept mutex would tell a
            // later start that this app runs while this start has no watcher to
            // answer its show request, so release whatever was created and start
            // without a claim.
            if let Ok(mutex) = mutex {
                // SAFETY: the mutex handle was just created and is not used again.
                let _ = unsafe { CloseHandle(mutex) };
            }
            if let Ok(event) = event {
                // SAFETY: the event handle was just created and is not used again.
                let _ = unsafe { CloseHandle(event) };
            }
            eprintln!("OpenSCL4: could not tell whether the app already runs; starting anyway");
            return Some(FirstInstance::unclaimed());
        }
    };
    if already {
        // SAFETY: the event handle was just opened and this start does not use
        // it again.
        if let Err(err) = unsafe { SetEvent(event) } {
            eprintln!("OpenSCL4: could not ask the running app to show itself: {err}");
        }
        eprintln!("OpenSCL4: already running; asked the running app to show its window");
        return None;
    }
    let show_request = ShowRequest::new();
    // The handle as a number the watcher thread can take along: `HANDLE` itself
    // is not `Send`.
    let event_number = event.0 as isize;
    let watcher = {
        let show_request = ShowRequest::clone(&show_request);
        std::thread::Builder::new()
            .name("single-instance".into())
            .spawn(move || watch(event_number, show_request))
    };
    if let Err(err) = watcher {
        eprintln!(
            "OpenSCL4: could not start the single-instance watcher; a later start will not show this window: {err}"
        );
    }
    Some(FirstInstance {
        _mutex: mutex,
        show_request,
    })
}

/// Waits for "show your window" requests and records each one, until the event
/// cannot be waited on any more.
fn watch(event_number: isize, show_request: ShowRequest) {
    loop {
        // SAFETY: the event handle is never closed while the app runs.
        let woken = unsafe { WaitForSingleObject(HANDLE(event_number as *mut _), INFINITE) };
        if woken != WAIT_OBJECT_0 {
            break;
        }
        show_request.request();
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU64;
    use std::time::{Duration, Instant};

    use super::*;

    /// A unique folder path for one test: the single-instance names live as
    /// long as the test process does, so every test claim gets its own.
    fn temp_folder(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "openscl4-single-{}-{sequence}-{label}",
            std::process::id()
        ))
    }

    #[test]
    fn the_folder_key_is_stable_and_case_insensitive() {
        let key = folder_key(Path::new(r"C:\Users\Me\AppData\Roaming\OpenSCL4"));
        assert_eq!(key.len(), 16, "the key is a fixed-width hash");
        assert_eq!(
            key,
            folder_key(Path::new(r"c:\users\me\appdata\roaming\openscl4")),
            "Windows folder names do not differ by case"
        );
        assert_ne!(
            key,
            folder_key(Path::new(r"C:\Users\Me\AppData\Roaming\Other"))
        );
    }

    #[test]
    fn the_instance_names_carry_the_folder_key() {
        let folder = Path::new(r"C:\Users\Me\AppData\Roaming\OpenSCL4");
        let key = folder_key(folder);
        let (mutex, event) = instance_names(folder);
        assert_eq!(mutex, format!(r"Local\OpenSCL4-{key}"));
        assert_eq!(event, format!(r"Local\OpenSCL4Show-{key}"));
    }

    #[test]
    fn the_first_start_claims_the_app_and_a_second_asks_it_to_show() {
        let folder = temp_folder("claim");
        let first = claim(&folder).expect("the first start claims the app");
        let show = first.show_request();
        assert!(!show.take_requested(), "no start has asked yet");

        assert!(
            claim(&folder).is_none(),
            "a second start finds the first and ends"
        );

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if show.take_requested() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the second start did not ask the first to show its window");
    }

    #[test]
    fn two_folders_are_two_instances() {
        let first = claim(&temp_folder("one")).expect("the first folder claims the app");
        let second = claim(&temp_folder("two")).expect("another folder is another instance");
        assert!(!first.show_request().take_requested());
        assert!(!second.show_request().take_requested());
    }

    #[test]
    fn a_half_created_claim_releases_the_app_for_a_later_start() {
        let folder = temp_folder("event-failure");
        let (_, event_name) = instance_names(&folder);
        // Occupy the event's name with a mutex of the same name: Windows refuses
        // to create an event under a name an object of another type already uses,
        // so this start gets its mutex but no event.
        // SAFETY: the name lives through the call and the handle is closed below.
        let blocker = unsafe { CreateMutexW(None, false, &HSTRING::from(event_name)) }
            .expect("the blocking object is created");
        let first = claim(&folder).expect("a failed claim still starts the app");
        assert!(!first.show_request().take_requested());
        // SAFETY: the blocker handle is owned by this test and is not used again.
        let _ = unsafe { CloseHandle(blocker) };

        // If the failed claim had kept its mutex, this start would find it and
        // end without a window; it must be able to claim the app instead.
        let second = claim(&folder).expect("a later start can still claim the app");
        assert!(!second.show_request().take_requested());
    }
}
