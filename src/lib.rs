mod eqregex;
mod file_io;
mod rules;
mod timestamps;

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use timestamps::{serialize_date, DATE_FORMAT};

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
    fn new(mapping: HashMap<String, String>) -> Result<Self> {
        let date = mapping
            .get("Date")
            .and_then(|x| NaiveDate::parse_from_str(x, DATE_FORMAT).ok())
            .ok_or(anyhow!(
                "Either a Date column is missing, or the value cannot be read as a date."
            ))?;
        let payee = mapping
            .get("Payee")
            .ok_or(anyhow!("A Payee column is missing."))?
            .to_owned();
        let category = mapping.get("Category").and_then(|x| Some(x.to_owned()));
        let memo = mapping.get("Memo").and_then(|x| Some(x.to_owned()));
        let amount = mapping
            .get("Amount")
            .and_then(|x| Decimal::from_str_exact(x).ok())
            .ok_or(anyhow!(
                "Either an Amount column is missing, or the value cannot be read as a decimal."
            ))?;
        let check = mapping.get("Check#").and_then(|x| x.parse().ok());
        let orig_payee = payee.to_owned();

        return Ok(NormalizedBankData {
            date,
            payee,
            category,
            memo,
            amount,
            check,
            orig_payee,
        });
    }
}

/// Convert a dollar amount in string form into a Decimal object.
///
/// Some banks express this in negated values, and if that is the case
/// the negate option can be used to re-interpret as positive.
pub fn interpret_dollar_amount(amount: &str, negate: bool) -> Decimal {
    // Convert the given value to a decimal,
    // defaulting to zero if it cannot be converted.
    let amt = Decimal::from_str_exact(amount).unwrap_or_default();

    // Return a negated version of the value if necessary.
    if negate {
        -amt
    } else {
        amt
    }
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
        wtr.serialize(NormalizedBankData {
            date: NaiveDate::parse_from_str("2024-01-01", DATE_FORMAT).unwrap(),
            payee: "MOD".to_string(),
            category: Some("Dining".to_string()),
            memo: None,
            amount: dec!(-15.32),
            check: None,
            orig_payee: "MOD PIZZA".to_string(),
        })
        .unwrap();
        wtr.serialize(NormalizedBankData {
            date: NaiveDate::parse_from_str("2024-02-01", DATE_FORMAT).unwrap(),
            payee: "ACE".to_string(),
            category: Some("Home:Maintenance".to_string()),
            memo: Some("Nails".to_string()),
            amount: dec!(-6.02),
            check: Some(123),
            orig_payee: "ACE HARDWARE CO".to_string(),
        })
        .unwrap();
        wtr.flush().unwrap();

        // Perform the test itself.
        let expected = "Date,Payee,Category,Memo,Amount,Check#\n\
                              2024-01-01,MOD,Dining,,-15.32,\n\
                              2024-02-01,ACE,Home:Maintenance,Nails,-6.02,123\n";
        let result = String::from_utf8(wtr.into_inner().unwrap()).unwrap();
        assert_eq!(result, expected);
    }
}
