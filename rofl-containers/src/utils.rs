use std::{fs, path::Path};

/// A helper struct that removes a given file when dropped.
pub struct RemoveFileOnDrop<P: AsRef<Path>> {
    path: P,
}

impl<P: AsRef<Path>> RemoveFileOnDrop<P> {
    /// Create a new instance.
    pub fn new(path: P) -> Self {
        Self { path }
    }
}

impl<P: AsRef<Path>> Drop for RemoveFileOnDrop<P> {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_remove_file_on_drop() {
        fs::write("/tmp/file", b"test").unwrap();
        assert!(fs::exists("/tmp/file").unwrap());

        let guard = RemoveFileOnDrop::new("/tmp/file");
        assert!(fs::exists("/tmp/file").unwrap());
        drop(guard);

        assert!(!fs::exists("/tmp/file").unwrap());
    }
}
