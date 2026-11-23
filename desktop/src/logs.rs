//! Small dependency-free application log used by Copy logs.
//!
//! Messages still go to stderr, while a bounded in-memory history and a rotating
//! file preserve real diagnostics for a windowed release with no console.

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

const KEPT_LINES: usize = 1000;
const MAX_FILE_BYTES: u64 = 1024 * 1024;

#[derive(Default)]
struct History {
    lines: VecDeque<String>,
}

impl History {
    fn push(&mut self, line: &str) {
        if line.is_empty() {
            return;
        }
        if self.lines.len() == KEPT_LINES {
            self.lines.pop_front();
        }
        self.lines.push_back(line.to_owned());
    }

    fn text(&self) -> String {
        let mut text = header();
        text.push('\n');
        for line in &self.lines {
            text.push_str(line);
            text.push('\n');
        }
        text
    }
}

static RECENT: LazyLock<Mutex<History>> = LazyLock::new(|| Mutex::new(History::default()));
static LOG_FILE: LazyLock<Mutex<Option<(PathBuf, File)>>> = LazyLock::new(|| Mutex::new(None));

fn header() -> String {
    format!(
        "OpenSCL4 {} ({}, {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn open_append(path: &Path) -> Option<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            let _ = writeln!(
                std::io::stderr(),
                "OpenSCL4: could not open {}: {error}; the log stays in memory",
                path.display()
            );
        })
        .ok()
}

/// Starts appending to `<config>/logs/openscl4.log`. A file error never stops
/// the app: stderr and the bounded in-memory history remain available.
pub fn open_file(config_directory: &Path) {
    let directory = config_directory.join("logs");
    if let Err(error) = fs::create_dir_all(&directory) {
        let _ = writeln!(
            std::io::stderr(),
            "OpenSCL4: could not create {}: {error}; the log stays in memory",
            directory.display()
        );
        return;
    }
    let path = directory.join("openscl4.log");
    let file = open_append(&path).map(|file| (path, file));
    *LOG_FILE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = file;
    write_file_line(&header());
    line("Application started".to_owned());
}

fn write_file_line(text: &str) {
    if text.is_empty() {
        return;
    }
    let mut slot = LOG_FILE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some((path, file)) = slot.as_mut() else {
        return;
    };
    if file
        .metadata()
        .is_ok_and(|metadata| metadata.len() >= MAX_FILE_BYTES)
    {
        let rotated = path.with_file_name("openscl4.log.1");
        let _ = fs::remove_file(&rotated);
        let _ = fs::rename(&*path, &rotated);
        match open_append(path) {
            Some(fresh) => *file = fresh,
            None => {
                *slot = None;
                return;
            }
        }
    }
    let _ = writeln!(file, "{text}");
    let _ = file.flush();
}

/// Records one already-formatted application message.
pub fn line(message: String) {
    let message = message.trim_end().to_owned();
    if message.is_empty() {
        return;
    }
    let _ = writeln!(std::io::stderr(), "{message}");
    write_file_line(&message);
    RECENT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(&message);
}

/// The real recent application messages, oldest first, with a version header.
pub fn recent() -> String {
    RECENT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .text()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_logs_have_a_header_and_keep_only_the_latest_lines() {
        let mut history = History::default();
        for index in 0..KEPT_LINES + 5 {
            history.push(&format!("line {index}"));
        }
        history.push("");
        let text = history.text();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("OpenSCL4 "));
        assert_eq!(lines.len(), KEPT_LINES + 1);
        assert_eq!(lines[1], "line 5");
        assert_eq!(lines[KEPT_LINES], format!("line {}", KEPT_LINES + 4));
    }
}
