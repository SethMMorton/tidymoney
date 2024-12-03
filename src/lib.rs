mod file_io;
mod rules;
mod timestamps;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use timestamps::serialize_date;

/// Container for bank data to be serialized into the normalized CSV.
#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct NormalizedBankData<'a> {
    #[serde(serialize_with = "serialize_date")]
    date: &'a NaiveDate,
    payee: &'a str,
    category: &'a str,
    memo: Option<&'a str>,
    amount: &'a Decimal,
    #[serde(rename = "Check#")]
    check: Option<u32>,
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

    use crate::timestamps::DATE_FORMAT;

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
            date: &NaiveDate::parse_from_str("2024-01-01", DATE_FORMAT).unwrap(),
            payee: "MOD",
            category: "Dining",
            memo: None,
            amount: &dec!(-15.32),
            check: None,
        })
        .unwrap();
        wtr.serialize(NormalizedBankData {
            date: &NaiveDate::parse_from_str("2024-02-01", DATE_FORMAT).unwrap(),
            payee: "ACE",
            category: "Home:Maintenance",
            memo: Some("Nails"),
            amount: &dec!(-6.02),
            check: Some(123),
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
