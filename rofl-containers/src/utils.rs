use std::{
    fs,
    io::{self, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};

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

/// Sanitize a filename by replacing invalid characters with underscores.
pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// The base directory for ephemeral ROFL state.
pub const ROFL_BASE_DIR: &str = "/run/rofl";

const ROFL_DIR_FILE_NAME: &str = "name";
const ROFL_DIR_FILE_VALUE: &str = "value";

/// A helper struct for managing ROFL ephemeral directories.
#[derive(Debug, Clone)]
pub struct RoflDir {
    base: PathBuf,
    namespaces: Vec<String>,
}

impl RoflDir {
    /// Create a new ephemeral ROFL directory with the given name.
    pub fn new(namespaces: &[&str]) -> Self {
        Self {
            base: PathBuf::from(ROFL_BASE_DIR),
            namespaces: namespaces.iter().map(|n| n.to_string()).collect(),
        }
    }

    /// Initialize the ROFL directory, removing any existing contents first.
    pub fn init(&self) -> Result<()> {
        let _ = fs::remove_dir_all(&self.base);
        fs::create_dir_all(&self.base)?;
        for namespace in &self.namespaces {
            fs::create_dir_all(self.base.join(namespace))?;
        }
        Ok(())
    }

    /// Set a value in the ROFL directory with the given name and value.
    pub fn set(&self, namespace: &str, name: &str, value: impl AsRef<[u8]>) -> Result<()> {
        let sane_name = sanitize_filename(name);
        let dir = self.base.join(namespace).join(&sane_name);
        fs::create_dir_all(&dir)?;

        // Ensure that we only overwrite the same entry and fail if this is a collision due to
        // sanitization of the name.
        match fs::read_to_string(dir.join(ROFL_DIR_FILE_NAME)) {
            Ok(existing_name) if existing_name != name => {
                return Err(anyhow!("filename sanitization would cause collision"));
            }
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                fs::write(dir.join(ROFL_DIR_FILE_NAME), name)?;
            }
            Err(_) => return Err(anyhow!("failed to read existing name")),
        }

        let mut temp_file = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o666))
            .tempfile_in(&dir)
            .map_err(|_| anyhow!("failed to create temporary file"))?;
        temp_file
            .write_all(value.as_ref())
            .map_err(|_| anyhow!("failed to write new value"))?;

        // Rename within the same directory so readers see either the complete old or new value.
        temp_file
            .persist(dir.join(ROFL_DIR_FILE_VALUE))
            .map_err(|_| anyhow!("failed to persist new value"))?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Read;

    #[test]
    fn test_remove_file_on_drop() {
        fs::write("/tmp/file", b"test").unwrap();
        assert!(fs::exists("/tmp/file").unwrap());

        let guard = RemoveFileOnDrop::new("/tmp/file");
        assert!(fs::exists("/tmp/file").unwrap());
        drop(guard);

        assert!(!fs::exists("/tmp/file").unwrap());
    }

    #[test]
    fn test_rofl_dir_atomic_update() {
        let base = std::env::temp_dir().join(format!(
            "rofl-dir-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        let rofl_dir = RoflDir {
            base: base.clone(),
            namespaces: vec!["env".to_string()],
        };
        rofl_dir.init().unwrap();
        rofl_dir.set("env", "TEST", "original value").unwrap();

        let dir = base.join("env/TEST");
        let mut old_reader = fs::File::open(dir.join(ROFL_DIR_FILE_VALUE)).unwrap();
        rofl_dir.set("env", "TEST", "new").unwrap();

        let mut old_value = String::new();
        old_reader.read_to_string(&mut old_value).unwrap();
        assert_eq!(old_value, "original value");
        assert_eq!(fs::read(dir.join(ROFL_DIR_FILE_VALUE)).unwrap(), b"new");

        rofl_dir.set("env", "TEST", "").unwrap();
        assert!(fs::read(dir.join(ROFL_DIR_FILE_VALUE)).unwrap().is_empty());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn test_sanitize_filename() {
        let cases = [
            ("", ""),
            ("abc", "abc"),
            ("abc_", "abc_"),
            ("abc_def", "abc_def"),
            ("abc-def", "abc_def"),
            ("abc_def-", "abc_def_"),
        ];
        for (input, expected) in cases {
            assert_eq!(sanitize_filename(input), expected, "input: {input:?}");
        }
    }
}
