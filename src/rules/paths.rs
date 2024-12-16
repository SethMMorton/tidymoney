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
    /// The path to the timestamps JSON file.
    #[serde(deserialize_with = "deserialize_path")]
    timestamps: PathBuf,
    /// The path to the directory where old and new CSV files will be stored.
    #[serde(deserialize_with = "deserialize_path")]
    storage: PathBuf,
}

impl AuxillaryPaths {
    /// Construct a new object - only needed for testing.
    #[cfg(test)]
    pub fn new<P: Into<PathBuf>, Q: Into<PathBuf>>(timestamps: P, storage: Q) -> Self {
        AuxillaryPaths {
            timestamps: timestamps.into(),
            storage: storage.into(),
        }
    }

    // Ensure the contained data is correct.
    pub fn validate(&self) -> Result<()> {
        // The storage directory must be a JSON file.
        if !self.timestamps.is_file() || self.timestamps.extension().is_some_and(|x| x != "json") {
            return Err(anyhow!(format!(
                "The timestamps path {} is not a JSON file.",
                self.timestamps.to_str().unwrap()
            )));
        }

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

    fn parse_toml(timestamps: &str, storage: &str) -> Result<AuxillaryPaths, toml::de::Error> {
        return toml::from_str(&format! {
        "timestamps = {:#?}\nstorage = {:#?}\n", timestamps, storage
        });
    }

    #[test]
    fn test_timestamps_must_exist() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let parsed = parse_toml(
            "/does/not/exist/file.json",
            temp.path().as_os_str().to_str().unwrap(),
        );
        assert!(parsed
            .err()
            .unwrap()
            .to_string()
            .contains("No such file or directory"));
    }

    #[test]
    fn test_timestamps_must_be_a_file() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let temp = temp.path().as_os_str().to_str().unwrap();
        let parsed = parse_toml(temp, temp);
        assert!(parsed
            .unwrap()
            .validate()
            .err()
            .unwrap()
            .to_string()
            .contains("is not a JSON file"));
    }

    #[test]
    fn test_timestamps_must_have_a_json_extension() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let csv = temp.path().join("file.csv");
        fs::write(&csv, "column").unwrap();
        let parsed = parse_toml(
            csv.as_os_str().to_str().unwrap(),
            temp.path().as_os_str().to_str().unwrap(),
        );
        assert!(parsed
            .unwrap()
            .validate()
            .err()
            .unwrap()
            .to_string()
            .contains("is not a JSON file"));
    }

    #[test]
    fn test_storage_must_exist() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let stamps = temp.path().join("file.json");
        fs::write(&stamps, "{}").unwrap();
        let parsed = parse_toml(stamps.as_os_str().to_str().unwrap(), "/does/not/exist");
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
        let parsed = parse_toml(&path, &path);
        assert!(parsed
            .unwrap()
            .validate()
            .err()
            .unwrap()
            .to_string()
            .contains("is not a directory"));
    }
}
