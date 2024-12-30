use std::path::PathBuf;

use anyhow::{anyhow, Result};
use expanduser::expanduser;
use serde::{Deserialize, Deserializer};

#[cfg(test)]
use std::convert::Into;

/// Paths used by the program for various purposes.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AuxillaryPaths {
    /// The path to the directory where old and new CSV files will be stored.
    #[serde(deserialize_with = "deserialize_path")]
    storage: PathBuf,
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
    expanduser(s)
        .map_err(serde::de::Error::custom)?
        .canonicalize()
        .map_err(serde::de::Error::custom)
}

#[cfg(test)]
mod test {
    use std::fs;

    use super::*;

    fn parse_toml(storage: &str) -> Result<AuxillaryPaths, toml::de::Error> {
        return toml::from_str(&format! {"storage = {:#?}\n", storage });
    }

    #[test]
    fn test_storage_must_exist() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let stamps = temp.path().join("file.json");
        fs::write(&stamps, "{}").unwrap();
        let parsed = parse_toml("/does/not/exist");
        assert!(parsed
            .err()
            .unwrap()
            .to_string()
            .contains("No such file or directory"));
    }

    #[test]
    fn test_storage_must_be_a_directory() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let stamps = temp.path().join("file.json");
        fs::write(&stamps, "{}").unwrap();
        let path = stamps.as_os_str().to_str().unwrap();
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
