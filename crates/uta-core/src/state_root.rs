//! The user state root (design §7.1 单实例: `OPENALICE_HOME`).
//!
//! Resolution follows OpenAlice (`design/investigation/existing-capabilities.md:239`):
//! `OPENALICE_HOME` if set, otherwise `~/.openalice`. The core's own files
//! (SQLite database and lock file) live in `<root>/data/uta/`.

use std::io;
use std::path::PathBuf;

pub fn core_dir() -> io::Result<PathBuf> {
    let root = match std::env::var_os("OPENALICE_HOME") {
        Some(home) if !home.is_empty() => PathBuf::from(home),
        _ => std::env::home_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?
            .join(".openalice"),
    };
    let dir = root.join("data").join("uta");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}
