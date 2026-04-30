use tempfile::{TempDir, tempdir};

pub fn temp_workspace() -> TempDir {
    tempdir().expect("temporary workspace should be created")
}
