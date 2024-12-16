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
    #[serde(default, rename = "IncomeOK")]
    income_ok: bool,
    /// The payee as originally given in the raw data.
    #[serde(default, deserialize_with = "deserialize_option_regex")]
    orig_payee: Option<EqRegex>,
}

impl CategoryAndMemoRules {
    /// Construct a new object - only needed for testing.
    #[cfg(test)]
    pub fn new(
        mapping: HashMap<String, String>,
        // orig_payee: Option<EqRegex>,
    ) -> Self {
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
        let income_ok = mapping.get("income_ok").is_some();
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
        if self.payee.as_ref().is_some_and(|p| *p == transaction.payee) {
            return false;
        }

        if self
            .orig_payee
            .as_ref()
            .is_some_and(|op| !op.is_match(&transaction.orig_payee))
        {
            return false;
        }

        if self
            .category
            .as_ref()
            .is_some_and(|cc| transaction.category.as_ref().is_some_and(|tc| *tc == *cc))
        {
            return false;
        }

        // TODO: Add IncomeOk

        let min_amt = self.min_amount.unwrap_or(Decimal::MIN);
        let max_amt = self.min_amount.unwrap_or(Decimal::MAX);
        if !(transaction.amount >= min_amt && transaction.amount <= max_amt) {
            return false;
        }

        // If the amount is not equal to the target it cannot be a match.
        if self.amount.is_some_and(|x| x != transaction.amount) {
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

    use crate::as_hashmap;

    #[test]
    fn test_check_at_least_one_affirmative() {
        let obj = CategoryAndMemoRules::new(as_hashmap(vec![("orig_payee", "Dennis")]));
        assert!(obj.check_at_least_one());
    }

    #[test]
    fn test_categories_must_give_at_least_one_rule() {
        let obj = CategoryAndMemoRules::new(as_hashmap(vec![]));
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
        let obj = CategoryAndMemoRules::new(as_hashmap(vec![]));
        assert!(!obj.check_at_least_one());
        assert_eq!(
            obj.validate("memo", "Sandwich").err().unwrap().to_string(),
            "The memo \"Sandwich\" must implement a rule."
        );
    }
}
