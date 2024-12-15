use std::hash::{Hash, Hasher};
use std::ops::Deref;

use regex::Regex;

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
