use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Deserializer};
use simple_expand_tilde::expand_tilde;

#[cfg(test)]
use std::convert::Into;

/// Expand '~' and cannoicalize the given path.
pub fn normalize_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    expand_tilde(path.as_ref())
        .ok_or_else(|| anyhow!("Cannot expand ~ to a home directory"))
}

/// Paths used by the program for various purposes.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AuxillaryPaths {
    /// The path to the directory where old and new CSV files will be stored.
    #[serde(deserialize_with = "deserialize_path")]
    pub storage: PathBuf,
}

impl AuxillaryPaths {
    /// Construct a new object - only needed for testing.
    #[cfg(test)]
    pub fn new(storage: impl Into<PathBuf>) -> Self {
        AuxillaryPaths {
            storage: storage.into(),
        }
    }

    // Ensure the contained data is correct.
    pub fn validate(&self) -> Result<()> {
        // The storage directory must be a directory.
        if !self.storage.is_dir() {
            return Err(anyhow!(format!(
                "The storage path {} is not a directory.",
                self.storage.to_str().unwrap()
            )));
        }

        Ok(())
    }
}

/// Instructions on how to deserialize a path object.
fn deserialize_path<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    normalize_path(s).map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod test {
    use std::fs;

    use super::*;

    fn parse_toml(storage: &str) -> Result<AuxillaryPaths, toml::de::Error> {
        return toml::from_str(&format! {"storage = {:#?}\n", storage });
    }

    #[test]
    fn test_normaize_path() {
        let given = "~/location";
        let result = normalize_path(given).unwrap();
        assert_ne!(result, PathBuf::from(given));
    }

    #[test]
    fn test_storage_must_exist() {
        let parsed = parse_toml("/does/not/exist");
        assert!(parsed
            .unwrap()
            .validate()
            .err()
            .unwrap()
            .to_string()
            .contains("is not a directory"));
    }

    #[test]
    fn test_storage_must_be_a_directory() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let storage = temp.path().join("file.json");
        fs::write(&storage, "{}").unwrap();
        let path = storage.as_os_str().to_str().unwrap();
        let parsed = parse_toml(&path);
        assert!(parsed
            .unwrap()
            .validate()
            .err()
            .unwrap()
            .to_string()
            .contains("is not a directory"));
    }
}
