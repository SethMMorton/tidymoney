use std::{collections::HashMap, fmt, marker::PhantomData, str::FromStr};

use anyhow::Result;
use regex::Regex;
use rust_decimal::Decimal;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

use crate::rules::date_filter::{date_is_outside_range, validate_date_filters};
use crate::rules::eqregex::{deserialize_regex, EqRegex};
use crate::NormalizedBankData;

/// Rules for specifying how to map a payee pattern to a specific payee.
/// The amount of the transaction can also be taken into account.
#[derive(Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
pub struct PayeeRules {
    /// The pattern to use to identify a payee.
    #[serde(deserialize_with = "deserialize_regex")]
    pattern: EqRegex,
    /// The minimum range that a transaction must be for to identify as this payee.
    min_amount: Option<Decimal>,
    /// The maximum range that a transaction must be for to identify as this payee.
    max_amount: Option<Decimal>,
    /// The exact amount that a transaction must be for to identify as this payee.
    amount: Option<Decimal>,
    /// The lowest date in the month that a transaction can have to identify as this payee.
    min_date_in_month: Option<u32>,
    /// The highest date in the month that a transaction can have to identify as this payee.
    max_date_in_month: Option<u32>,
    /// The lowest date in the year that a transaction can have to identify as this payee.
    min_date_in_year: Option<(u32, u32)>,
    /// The highest date in the year that a transaction can have to identify as this payee.
    max_date_in_year: Option<(u32, u32)>,
}

impl PayeeRules {
    /// Construct a new object - only needed for testing.
    #[cfg(test)]
    pub fn new(mapping: HashMap<String, String>) -> Self {
        let pattern = mapping
            .get("pattern")
            .and_then(|x| Some(EqRegex(Regex::new(x).unwrap())))
            .unwrap();
        let min_amount = mapping
            .get("min_amount")
            .and_then(|x| Decimal::from_str_exact(x).ok());
        let max_amount = mapping
            .get("max_amount")
            .and_then(|x| Decimal::from_str_exact(x).ok());
        let amount = mapping
            .get("amount")
            .and_then(|x| Decimal::from_str_exact(x).ok());
        let (min_date_in_month, max_date_in_month, min_date_in_year, max_date_in_year) =
            crate::rules::date_filter::process_date_filter_mapping(mapping);
        PayeeRules {
            pattern,
            min_amount,
            max_amount,
            amount,
            min_date_in_month,
            max_date_in_month,
            min_date_in_year,
            max_date_in_year,
        }
    }

    /// Determine if the given transaction matches this set of rules.
    pub fn transaction_matches(&self, transaction: &NormalizedBankData) -> bool {
        // If the amount does not fall in the value ranges it cannot be a match.
        let min_amt = self.min_amount.unwrap_or(Decimal::ZERO).abs();
        let max_amt = self.max_amount.unwrap_or(Decimal::MAX).abs();
        let amt = transaction.amount.abs();
        if !(amt >= min_amt && amt <= max_amt) {
            return false;
        }

        // If the amount is not equal to the target it cannot be a match.
        if self.amount.is_some_and(|x| x.abs() != amt) {
            return false;
        }

        // If the payee does not match the pattern it cannot be a match.
        if !self.pattern.is_match(&transaction.orig_payee) {
            return false;
        }

        // Ensure the dates are within the required ranges.
        if date_is_outside_range(
            &transaction.date,
            (self.min_date_in_month, self.max_date_in_month),
            (self.min_date_in_year, self.max_date_in_year),
        ) {
            return false;
        }

        true
    }

    /// Ensure the given rules are semantically correct.
    pub fn validate(&self, name: &str) -> Result<()> {
        validate_date_filters(
            "payee",
            name,
            (self.min_date_in_month, self.max_date_in_month),
            (self.min_date_in_year, self.max_date_in_year),
        )
    }
}

/// Create this PayeeRules from a string.
impl FromStr for PayeeRules {
    type Err = void::Void;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PayeeRules {
            pattern: EqRegex(Regex::new(s).unwrap_or_else(|_| {
                panic!("Could not parse the string {s} as a regular expression")
            })),
            min_amount: None,
            max_amount: None,
            amount: None,
            min_date_in_month: None,
            max_date_in_month: None,
            min_date_in_year: None,
            max_date_in_year: None,
        })
    }
}

/// Teach serde how to read some field as a string or a struct.
/// Lifted from https://serde.rs/string-or-struct.html
fn string_or_struct<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + FromStr<Err = void::Void>,
    D: Deserializer<'de>,
{
    // This is a Visitor that forwards string types to T's `FromStr` impl and
    // forwards map types to T's `Deserialize` impl. The `PhantomData` is to
    // keep the compiler from complaining about T being an unused generic type
    // parameter. We need T in order to know the Value type for the Visitor
    // impl.
    struct StringOrStruct<T>(PhantomData<fn() -> T>);

    impl<'de, T> Visitor<'de> for StringOrStruct<T>
    where
        T: Deserialize<'de> + FromStr<Err = void::Void>,
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("string or map")
        }

        fn visit_str<E>(self, value: &str) -> Result<T, E>
        where
            E: serde::de::Error,
        {
            Ok(FromStr::from_str(value).unwrap())
        }

        fn visit_map<M>(self, map: M) -> Result<T, M::Error>
        where
            M: MapAccess<'de>,
        {
            // `MapAccessDeserializer` is a wrapper that turns a `MapAccess`
            // into a `Deserializer`, allowing it to be used as the input to T's
            // `Deserialize` implementation. T then deserializes itself using
            // the entries from the map visitor.
            Deserialize::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_any(StringOrStruct(PhantomData))
}

/// Function to tell how to deserialize PayeeRules from either a map,
/// string, or vector of maps or strings.
pub fn hashmap_payee_rules<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, Vec<PayeeRules>>, D::Error>
where
    D: Deserializer<'de>,
{
    /// Function to tell how to deserialize PayeeRules from a sequence.
    fn vec_payee_rules<'de, D>(deserializer: D) -> Result<Vec<PayeeRules>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(#[serde(deserialize_with = "string_or_struct")] PayeeRules);

        let v = Vec::deserialize(deserializer)?;
        Ok(v.into_iter().map(|Wrapper(a)| a).collect())
    }

    // Deserializer in either sequence or a scalar.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wrapper {
        #[serde(deserialize_with = "vec_payee_rules")]
        VecForm(Vec<PayeeRules>),
        #[serde(deserialize_with = "string_or_struct")]
        ScalarForm(PayeeRules),
    }

    // Choose the correct deserializer based on the data format.
    let v = HashMap::<String, Wrapper>::deserialize(deserializer)?;
    Ok(v.into_iter()
        .map(|(k, v)| match v {
            Wrapper::VecForm(seq) => (k, seq),
            Wrapper::ScalarForm(scalar) => (k, vec![scalar]),
        })
        .collect())
}

#[cfg(test)]
mod test {
    use super::*;

    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use crate::as_hashmap;

    #[rstest]
    #[case(
        vec![("pattern", "ACE")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "Target")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_amount", "10.00"), ("max_amount", "20.00")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_amount", "10.00"), ("max_amount", "15.00")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("amount", "15.43")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("amount", "15.00")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_month", "6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_month", "2")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("max_date_in_month", "6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("max_date_in_month", "2")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_month", "2"), ("max_date_in_month", "6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_month", "6"), ("max_date_in_month", "10")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_month", "25"), ("max_date_in_month", "6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_month", "25"), ("max_date_in_month", "2")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_month", "25"), ("max_date_in_month", "6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-29"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_month", "25"), ("max_date_in_month", "6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-24"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_year", "5/6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_year", "3/6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("max_date_in_year", "5/6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("max_date_in_year", "3/6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_year", "3/6"), ("max_date_in_year", "5/6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_year", "5/6"), ("max_date_in_year", "8/10")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_year", "12/3"), ("max_date_in_year", "5/6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_year", "12/3"), ("max_date_in_year", "3/6")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_year", "12/3"), ("max_date_in_year", "3/6")],
        vec![("Payee", "ACE"), ("Date", "2024-12-29"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("pattern", "ACE"), ("min_date_in_year", "12/3"), ("max_date_in_year", "3/6")],
        vec![("Payee", "ACE"), ("Date", "2024-11-24"), ("Amount", "-15.43")],
        false,
    )]
    fn test_transaction_matches(
        #[case] given: Vec<(&str, &str)>,
        #[case] txn_data: Vec<(&str, &str)>,
        #[case] expected: bool,
    ) {
        let transaction = NormalizedBankData::new(as_hashmap(txn_data));
        let result = PayeeRules::new(as_hashmap(given)).transaction_matches(&transaction);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_validate_works() {
        let given = vec![("pattern", "ACE"), ("max_date_in_year", "3/40")];
        let result = PayeeRules::new(as_hashmap(given)).validate("test");
        assert!(result.is_err());
    }
}
