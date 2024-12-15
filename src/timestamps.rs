use std::collections::HashMap;

use anyhow::Result;
use chrono::NaiveDate;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const DATE_FORMAT: &'static str = "%Y-%m-%d";

/// Instructions on how to serialize a date object.
pub fn serialize_date<S>(dt: &NaiveDate, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    dt.format(DATE_FORMAT).to_string().serialize(serializer)
}

/// Instructions on how to deserialize a date object.
pub fn deserialize_date<'de, D>(deserializer: D) -> Result<NaiveDate, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let dt = NaiveDate::parse_from_str(&s, DATE_FORMAT).map_err(serde::de::Error::custom)?;
    Ok(dt)
}

/// Container for dates as they are read from the rules file.
#[derive(Debug, Serialize, Deserialize)]
struct AccountDate {
    account: String,
    #[serde(
        serialize_with = "serialize_date",
        deserialize_with = "deserialize_date"
    )]
    date: NaiveDate,
}

/// The TimestampKeeper keeps track of the most recent timestamps
/// seen for each account, and in doing so enables filtering entries
/// already seen.
#[derive(Debug)]
pub struct TimestampKeeper {
    dates: HashMap<String, NaiveDate>,
}

impl TimestampKeeper {
    /// Create a new TimestampKeeper by reading a raw JSON string.
    pub fn new(raw_data: &str) -> Result<Self> {
        let dates_as_vec: Vec<AccountDate> = serde_json::from_str(raw_data)?;
        let dates: HashMap<String, NaiveDate> = dates_as_vec
            .iter()
            .map(|element| (element.account.to_owned(), element.date))
            .collect();
        Ok(TimestampKeeper { dates })
    }

    /// Retrieve the updated timestamps as a (sorted) JSON string.
    pub fn get_updated_stamps(&self) -> Result<String, serde_json::Error> {
        let mut dates_as_vec: Vec<AccountDate> = self
            .dates
            .iter()
            .map(|(key, value)| AccountDate {
                account: key.to_owned(),
                date: *value,
            })
            .collect();
        dates_as_vec.sort_by(|x1, x2| x1.account.cmp(&x2.account));
        serde_json::to_string_pretty(&dates_as_vec)
    }

    /// Update the date stored for a given account if it is later than the stored date.
    pub fn update_date(&mut self, account: &str, date: &NaiveDate) {
        let this_date = self
            .dates
            .entry(account.to_owned())
            .or_insert(TimestampKeeper::early());
        if date > this_date {
            self.dates.insert(account.to_string(), *date);
        }
    }

    /// An early date.
    fn early() -> NaiveDate {
        NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use indoc::indoc;

    #[test]
    fn test_timestamp_tracking() {
        let given = indoc! { r#"
        [
            {
                "account": "VISA",
                "date": "2023-03-15"
            },
            {
                "account": "PNC",
                "date": "2024-01-04"
            }
        ]
        "#};
        let expected = indoc! {r#"
        [
          {
            "account": "Credit Union",
            "date": "2024-10-23"
          },
          {
            "account": "PNC",
            "date": "2024-01-04"
          },
          {
            "account": "VISA",
            "date": "2024-05-07"
          }
        ]"#};
        let mut stamps = TimestampKeeper::new(given).unwrap();
        stamps.update_date("VISA", &NaiveDate::from_ymd_opt(2024, 2, 29).unwrap());
        stamps.update_date("PNC", &NaiveDate::from_ymd_opt(2023, 7, 3).unwrap());
        stamps.update_date(
            "Credit Union",
            &NaiveDate::from_ymd_opt(2024, 10, 23).unwrap(),
        );
        stamps.update_date("VISA", &NaiveDate::from_ymd_opt(2024, 5, 7).unwrap());
        let result = stamps.get_updated_stamps().unwrap();
        assert_eq!(result, expected);
    }
}
