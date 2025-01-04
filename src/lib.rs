mod file_io;
mod process;
mod rules;
mod timestamps;

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::timestamps::serialize_date;

pub use crate::file_io::store_raw_transactions;
pub use crate::process::{
    account_for_dates_in_transactions, process_csv_files, write_transactions_to_file,
};
pub use crate::rules::{normalize_path, RuleFileData};
pub use crate::timestamps::{timestamps_path, TimestampKeeper, DATE_FORMAT};

/// Container for bank data to be serialized into the normalized CSV.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct NormalizedBankData {
    #[serde(serialize_with = "serialize_date")]
    pub date: NaiveDate,
    pub payee: String,
    pub category: Option<String>,
    pub memo: Option<String>,
    pub amount: Decimal,
    #[serde(rename = "Check#")]
    pub check: Option<u32>,
    #[serde(skip_serializing)]
    pub orig_payee: String,
}

impl NormalizedBankData {
    #[cfg(test)]
    fn new(mapping: HashMap<String, String>) -> Self {
        Self::from_raw_data(mapping, false, DATE_FORMAT, "testing").unwrap()
    }

    /// Instantiate this struct from raw data from file.
    pub fn from_raw_data(
        mapping: HashMap<String, String>,
        negate: bool,
        date_fmt: impl AsRef<str>,
        label: impl AsRef<str>,
    ) -> Result<Self> {
        // Get required columns.
        let label_str = label.as_ref();
        let payee_str = mapping.get("Payee").ok_or_else(|| {
            anyhow!(format!(
                "The account '{label_str}' is missing the Payee column"
            ))
        })?;
        let date_str = mapping.get("Date").ok_or_else(|| {
            anyhow!(format!(
                "The account '{label_str}' is missing the Date column"
            ))
        })?;
        let amount_str = mapping.get("Amount").ok_or_else(|| {
            anyhow!(format!(
                "The account '{label_str}' is missing the Amount column"
            ))
        })?;

        // Calculate the values of all the fields and return.
        return Ok(NormalizedBankData {
            date: NaiveDate::parse_from_str(date_str, date_fmt.as_ref()).or(Err(anyhow!(
                "Cannot parse the date {:#?} with the format string {:#?}",
                date_str,
                date_fmt.as_ref()
            )))?,
            payee: payee_str.to_owned(),
            category: mapping.get("Category").map(|x| x.to_owned()),
            memo: mapping.get("Memo").map(|x| x.to_owned()),
            amount: interpret_dollar_amount(amount_str, negate),
            check: mapping.get("Check#").and_then(|x| x.parse().ok()),
            orig_payee: payee_str.to_owned(),
        });
    }

    /// Determine if this transaction needs to be skipped.
    pub fn skipme(&self, start_date: &NaiveDate, end_date: &NaiveDate) -> bool {
        self.amount == Decimal::ZERO || self.date < *start_date || self.date > *end_date
    }
}

/// Convert a dollar amount in string form into a Decimal object.
///
/// Some banks express this in negated values, and if that is the case
/// the negate option can be used to re-interpret as positive.
fn interpret_dollar_amount(amount: impl AsRef<str>, negate: bool) -> Decimal {
    // Convert the given value to a decimal,
    // defaulting to zero if it cannot be converted.
    let amt = Decimal::from_str_exact(amount.as_ref()).unwrap_or_default();

    // Return a negated version of the value if necessary.
    if negate {
        -amt
    } else {
        amt
    }
}

/// Test helper function for converting vectors to hashmaps.
pub fn as_hashmap(data: Vec<(impl Into<String>, impl Into<String>)>) -> HashMap<String, String> {
    data.into_iter()
        .map(|(x, y)| (x.into(), y.into()))
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;

    use rstest::rstest;
    use rust_decimal_macros::dec;

    #[rstest]
    #[case("4.56", false, dec!(4.56))]
    #[case("-4.56", false, dec!(-4.56))]
    #[case("4.56", true, dec!(-4.56))]
    #[case("-4.56", true, dec!(4.56))]
    #[case("gandalf", false, dec!(0.00))]
    #[case("gandalf", true, dec!(0.00))]
    fn test_interpret_dollar_amount(
        #[case] given: &str,
        #[case] negate: bool,
        #[case] expected: Decimal,
    ) {
        let result = interpret_dollar_amount(given, negate);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_csv_serialization() {
        // Use a vector buffer as an in-memory sink.
        let mut wtr = csv::Writer::from_writer(vec![]);

        // Write two entries to the writer.
        wtr.serialize(NormalizedBankData::new(as_hashmap(vec![
            ("Date", "2024-01-01"),
            ("Payee", "MOD"),
            ("Category", "Dining"),
            ("Amount", "-15.32"),
            ("Check#", "one"),
        ])))
        .unwrap();
        wtr.serialize(NormalizedBankData::new(as_hashmap(vec![
            ("Date", "2024-02-01"),
            ("Payee", "ACE"),
            ("Category", "Home:Maintenance"),
            ("Memo", "Nails"),
            ("Amount", "-6.02"),
            ("Check#", "123"),
            ("OrigPayee", "ACE HARDWARE CO"),
        ])))
        .unwrap();
        wtr.flush().unwrap();

        // Perfor the test itself.
        let expected = "Date,Payee,Category,Memo,Amount,Check#\n\
                              2024-01-01,MOD,Dining,,-15.32,\n\
                              2024-02-01,ACE,Home:Maintenance,Nails,-6.02,123\n";
        let result = String::from_utf8(wtr.into_inner().unwrap()).unwrap();
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(
        vec![
            ("Date", "2024-01-15"),
            ("Payee", "MOD"),
            ("Amount", "0.00"),
        ],
        true,
    )]
    #[case(
        vec![
            ("Date", "2023-12-31"),
            ("Payee", "MOD"),
            ("Amount", "-15.32"),
        ],
        true,
    )]
    #[case(
        vec![
            ("Date", "2024-02-02"),
            ("Payee", "MOD"),
            ("Amount", "-15.32"),
        ],
        true,
    )]
    #[case(
        vec![
            ("Date", "2024-01-01"),
            ("Payee", "MOD"),
            ("Amount", "-15.32"),
        ],
        false
    )]
    fn test_skipme(#[case] given: Vec<(&str, &str)>, #[case] expected: bool) {
        let start_date = NaiveDate::parse_from_str("2024-01-01", &DATE_FORMAT).unwrap();
        let end_date = NaiveDate::parse_from_str("2024-02-01", &DATE_FORMAT).unwrap();
        let result = NormalizedBankData::new(as_hashmap(given)).skipme(&start_date, &end_date);
        assert_eq!(result, expected);
    }
}
