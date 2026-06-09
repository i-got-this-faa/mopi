use camino::Utf8PathBuf;
use std::fs;
use std::io;

/// Isolated XDG environment for benchmarks.
///
/// Creates a temporary directory with `config`, `data`, `cache`, and `runtime`
/// subdirectories so benchmarks do not touch the user's real directories.
///
/// # Cleaning
///
/// The temporary directory is removed when `BenchEnv` is dropped.
pub struct BenchEnv {
    _root: tempfile::TempDir,
    pub config_home: Utf8PathBuf,
    pub data_home: Utf8PathBuf,
    pub cache_home: Utf8PathBuf,
    pub runtime_dir: Utf8PathBuf,
}

impl BenchEnv {
    pub fn new() -> io::Result<Self> {
        let root = tempfile::tempdir()?;
        let root_path = root.path().to_path_buf();

        fn make_subdir(root: &std::path::Path, name: &str) -> io::Result<Utf8PathBuf> {
            let dir = root.join(name);
            fs::create_dir_all(&dir)?;
            Utf8PathBuf::from_path_buf(dir).map_err(|p| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("non-utf8 path: {}", p.display()),
                )
            })
        }

        let config_home = make_subdir(&root_path, "config")?;
        let data_home = make_subdir(&root_path, "data")?;
        let cache_home = make_subdir(&root_path, "cache")?;
        let runtime_dir = make_subdir(&root_path, "runtime")?;

        Ok(BenchEnv {
            _root: root,
            config_home,
            data_home,
            cache_home,
            runtime_dir,
        })
    }

    /// Return the root temporary directory path.
    pub fn root(&self) -> &std::path::Path {
        self._root.path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_env_creates_directories() {
        let env = BenchEnv::new().expect("bench env creation");
        assert!(
            env.config_home.as_std_path().is_dir(),
            "config dir exists"
        );
        assert!(env.data_home.as_std_path().is_dir(), "data dir exists");
        assert!(env.cache_home.as_std_path().is_dir(), "cache dir exists");
        assert!(env.runtime_dir.as_std_path().is_dir(), "runtime dir exists");
    }

    #[test]
    fn bench_env_directories_are_unique() {
        let a = BenchEnv::new().expect("env a");
        let b = BenchEnv::new().expect("env b");
        assert_ne!(a.config_home, b.config_home);
        assert_ne!(a.data_home, b.data_home);
    }

    #[test]
    fn bench_env_root_is_accessible() {
        let env = BenchEnv::new().expect("bench env creation");
        assert!(env.root().is_dir());
    }

}
