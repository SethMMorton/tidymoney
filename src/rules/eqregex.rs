use std::hash::{Hash, Hasher};
use std::ops::Deref;

use regex::Regex;
use serde::{Deserialize, Deserializer};

/// A regex object that can be tested for equality and used as a HashMap key.
#[derive(Debug)]
pub struct EqRegex(pub Regex);

impl Hash for EqRegex {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_str().hash(state);
    }
}

impl PartialEq for EqRegex {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for EqRegex {}

impl From<Regex> for EqRegex {
    fn from(regex: Regex) -> Self {
        Self(regex)
    }
}

impl Deref for EqRegex {
    type Target = Regex;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Instructions on how to deserialize a regex object.
pub fn deserialize_regex<'de, D>(deserializer: D) -> Result<EqRegex, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let regex = Regex::new(&s).map_err(serde::de::Error::custom)?;
    Ok(EqRegex::from(regex))
}

/// Instructions on how to deserialize an option regex object.
pub fn deserialize_option_regex<'de, D>(deserializer: D) -> Result<Option<EqRegex>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Option::<String>::deserialize(deserializer)?;
    match s {
        Some(s) => {
            let regex = Regex::new(&s).map_err(serde::de::Error::custom)?;
            Ok(Some(EqRegex::from(regex)))
        }
        None => {
            Ok(None)
        }
    }
}
