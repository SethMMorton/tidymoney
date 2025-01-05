use std::collections::HashMap;

use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer};

use crate::rules::eqregex::{deserialize_option_regex, EqRegex};
use crate::NormalizedBankData;

#[cfg(test)]
use regex::Regex;

/// Rules for specifying how to identify a category or memo for
/// a given transaction.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
pub struct CategoryAndMemoRules {
    /// The current name of this payee.
    payee: Option<String>,
    /// The current category of this payee.
    category: Option<String>,
    /// The amount of the transaction.
    amount: Option<Decimal>,
    /// The lower range of the transaction amount.
    min_amount: Option<Decimal>,
    /// The upper range of the transaction amount.
    max_amount: Option<Decimal>,
    /// Whether or not the amount can be income.
    #[serde(default = "true_value", rename = "IncomeOK")]
    income_ok: bool,
    /// The payee as originally given in the raw data.
    #[serde(default, deserialize_with = "deserialize_option_regex")]
    orig_payee: Option<EqRegex>,
}

/// The TRUTH!
fn true_value() -> bool {
    true
}

impl CategoryAndMemoRules {
    /// Construct a new object - only needed for testing.
    #[cfg(test)]
    pub fn new(mapping: HashMap<String, String>) -> Self {
        let payee = mapping.get("payee").and_then(|x| Some(x.to_owned()));
        let category = mapping.get("category").and_then(|x| Some(x.to_owned()));
        let amount = mapping
            .get("amount")
            .and_then(|x| Decimal::from_str_exact(x).ok());
        let min_amount = mapping
            .get("min_amount")
            .and_then(|x| Decimal::from_str_exact(x).ok());
        let max_amount = mapping
            .get("max_amount")
            .and_then(|x| Decimal::from_str_exact(x).ok());
        let income_ok = mapping
            .get("income_ok")
            .is_none_or(|x| x.to_lowercase() == "true");
        let orig_payee = mapping
            .get("orig_payee")
            .and_then(|x| Some(EqRegex(Regex::new(x).unwrap())));
        CategoryAndMemoRules {
            payee,
            category,
            amount,
            min_amount,
            max_amount,
            income_ok,
            orig_payee,
        }
    }

    /// Check if there is at least one item given for this object.
    fn check_at_least_one(&self) -> bool {
        self.payee.is_some()
            || self.category.is_some()
            || self.min_amount.is_some()
            || self.max_amount.is_some()
            || self.amount.is_some()
            || self.orig_payee.is_some()
    }

    /// Determine if the given transaction matches this set of rules.
    pub fn transaction_matches(&self, transaction: &NormalizedBankData) -> bool {
        // If a payee is provided and it does not match then
        // this transaction does not match.
        if self.payee.as_ref().is_some_and(|p| *p != transaction.payee) {
            return false;
        }

        // If an original payee pattern is provided and it does not match then
        // this transaction does not match.
        if self
            .orig_payee
            .as_ref()
            .is_some_and(|op| !op.is_match(&transaction.orig_payee))
        {
            return false;
        }

        // If a category pattern is provided and it does not match then
        // this transaction does not match. If no transaction category is provided
        // by default it cannot match.
        let cat = transaction.category.as_ref();
        if self
            .category
            .as_ref()
            .is_some_and(|cc| cat.is_none() || cat.is_some_and(|tc| *tc != *cc))
        {
            return false;
        }

        // If income is not OK but this is income then this transaction does not match.
        if !self.income_ok && transaction.amount > Decimal::ZERO {
            return false;
        }

        // If a min or max transaction value is provided and is not in the range then
        // this transaction does not match. Express in absolute value for user ease.
        let min_amt = self.min_amount.unwrap_or(Decimal::ZERO).abs();
        let max_amt = self.max_amount.unwrap_or(Decimal::MAX).abs();
        let amt = transaction.amount.abs();
        if !(amt >= min_amt && amt <= max_amt) {
            return false;
        }

        // If the amount is not equal to the target the transcaction does not match.
        // Express in absolute value for user ease.
        if self.amount.is_some_and(|x| x.abs() != amt) {
            return false;
        }

        true
    }

    /// Ensure the given rules are semantically correct.
    pub fn validate(&self, obj_type: &str, name: &str) -> Result<()> {
        if !self.check_at_least_one() {
            return Err(anyhow!(format!(
                "The {obj_type} {name:#?} must implement a rule."
            )));
        }
        Ok(())
    }
}

/// Function to tell how to deserialize CategoryAndMemoRules from either a map,
/// or vector of maps.
pub fn hashmap_cat_memo_rules<'de, D>(
    deserializer: D,
) -> Result<Option<HashMap<String, Vec<CategoryAndMemoRules>>>, D::Error>
where
    D: Deserializer<'de>,
{
    /// Function to tell how to deserialize CategoryAndMemoRules from a sequence.
    fn vec_cat_memo_rules<'de, D>(deserializer: D) -> Result<Vec<CategoryAndMemoRules>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(CategoryAndMemoRules);

        let v = Vec::deserialize(deserializer)?;
        Ok(v.into_iter().map(|Wrapper(a)| a).collect())
    }

    // Deserializer in either sequence or a scalar.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wrapper {
        #[serde(deserialize_with = "vec_cat_memo_rules")]
        VecForm(Vec<CategoryAndMemoRules>),
        ScalarForm(CategoryAndMemoRules),
    }

    // Choose the correct deserializer based on the data format.
    let v = HashMap::<String, Wrapper>::deserialize(deserializer)?;
    Ok(Some(
        v.into_iter()
            .map(|(k, v)| match v {
                Wrapper::VecForm(seq) => (k, seq),
                Wrapper::ScalarForm(scalar) => (k, vec![scalar]),
            })
            .collect(),
    ))
}

#[cfg(test)]
mod test {
    use super::*;

    use pretty_assertions::assert_eq;
    use rstest::rstest;

    use crate::as_hashmap;

    #[test]
    fn test_check_at_least_one_affirmative() {
        let obj = CategoryAndMemoRules::new(as_hashmap(vec![("orig_payee", "Dennis")]));
        assert!(obj.check_at_least_one());
    }

    #[test]
    fn test_categories_must_give_at_least_one_rule() {
        let obj = CategoryAndMemoRules::new(HashMap::new());
        assert!(!obj.check_at_least_one());
        assert_eq!(
            obj.validate("category", "Dining")
                .err()
                .unwrap()
                .to_string(),
            "The category \"Dining\" must implement a rule."
        );
    }

    #[test]
    fn test_memos_must_give_at_least_one_rule() {
        let obj = CategoryAndMemoRules::new(HashMap::new());
        assert!(!obj.check_at_least_one());
        assert_eq!(
            obj.validate("memo", "Sandwich").err().unwrap().to_string(),
            "The memo \"Sandwich\" must implement a rule."
        );
    }

    #[rstest]
    #[case(
        vec![("payee", "ACE")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("payee", "Target")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("orig_payee", "^ACE")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("orig_payee", "^TARGET")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("category", "Hardware")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("category", "Hardware")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43"), ("Category", "Hardware")],
        true,
    )]
    #[case(
        vec![("category", "Hardware")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43"), ("Category", "Garden")],
        false,
    )]
    #[case(
        vec![("min_amount", "10.00"), ("max_amount", "20.00")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("min_amount", "10.00"), ("max_amount", "15.00")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    #[case(
        vec![("income_ok", "true")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "15.43")],
        true,
    )]
    #[case(
        vec![],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "15.43")],
        true,
    )]
    #[case(
        vec![("income_ok", "false")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "15.43")],
        false,
    )]
    #[case(
        vec![("amount", "15.43")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        true,
    )]
    #[case(
        vec![("amount", "15.00")],
        vec![("Payee", "ACE"), ("Date", "2024-04-03"), ("Amount", "-15.43")],
        false,
    )]
    fn test_transaction_matches(
        #[case] given: Vec<(&str, &str)>,
        #[case] txn_data: Vec<(&str, &str)>,
        #[case] expected: bool,
    ) {
        let transaction = NormalizedBankData::new(as_hashmap(txn_data));
        let result = CategoryAndMemoRules::new(as_hashmap(given)).transaction_matches(&transaction);
        assert_eq!(result, expected);
    }
}
