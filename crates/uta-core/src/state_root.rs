//! The user state root (design core/core-process/design.md §4.6 统一路径文件: `OPENALICE_HOME`).
//!
//! Resolution matches Alice exactly (`design/investigation/existing-capabilities.md:239`,
//! OpenAlice `src/core/paths.ts`): `OPENALICE_HOME` when the variable is set,
//! even to an empty string (Alice uses `??`), otherwise `~/.openalice`; the
//! result is resolved against the working directory and normalized lexically
//! like Node's `path.resolve` (`..` removes the previous component without
//! consulting the filesystem). Both processes therefore name the same root
//! for the same environment and working directory. The core's own files
//! (SQLite database and lock file) live in `<root>/data/uta/`.

use std::io;
use std::path::{Component, Path, PathBuf};

pub fn core_dir() -> io::Result<PathBuf> {
    let root = match std::env::var_os("OPENALICE_HOME") {
        Some(home) => PathBuf::from(home),
        None => std::env::home_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no home directory"))?
            .join(".openalice"),
    };
    let cwd = std::env::current_dir()?;
    let dir = resolve_lexically(&cwd, &root.join("data").join("uta"));
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// `path.resolve(cwd, path)`: absolute paths replace `cwd`; `.` is dropped and
/// `..` removes the previous component, without touching the filesystem.
fn resolve_lexically(cwd: &Path, path: &Path) -> PathBuf {
    let joined = cwd.join(path);
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(all(test, unix))]
mod tests {
    use super::resolve_lexically;
    use std::path::Path;

    #[test]
    fn resolves_like_node_path_resolve() {
        let cwd = Path::new("/work/dir");
        assert_eq!(
            resolve_lexically(cwd, Path::new("")),
            Path::new("/work/dir")
        );
        assert_eq!(
            resolve_lexically(cwd, Path::new("a/link/../data")),
            Path::new("/work/dir/a/data")
        );
        assert_eq!(
            resolve_lexically(cwd, Path::new("/abs/./x")),
            Path::new("/abs/x")
        );
        assert_eq!(
            resolve_lexically(cwd, Path::new("../../..")),
            Path::new("/")
        );
    }
}
