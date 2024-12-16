use std::collections::HashMap;

use serde::Deserialize;

/// Rules for how to identify CSV columns to accounts, and how
/// to map those column names to output column names.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MappingRulesCsv {
    /// The label to give this account type.
    label: String,
    /// The columns to expect from this account type.
    identify: Vec<String>,
    /// How to map the given column names to output column names.
    translate: Option<RemapValuesCsv>,
}

impl MappingRulesCsv {
    /// Construct a new object - only needed for testing.
    #[cfg(test)]
    pub fn new(
        label: String,
        identify: Vec<String>,
        translate: HashMap<String, String>,
    ) -> MappingRulesCsv {
        let payee = translate.get("payee");
        let date = translate.get("date");
        let amount = translate.get("amount");
        let category = translate.get("category");
        let memo = translate.get("memo");
        let check = translate.get("check");
        MappingRulesCsv {
            label,
            identify,
            translate: (!translate.is_empty()).then_some(RemapValuesCsv {
                payee: payee.and_then(|x| Some(x.to_owned())),
                date: date.and_then(|x| Some(x.to_owned())),
                amount: amount.and_then(|x| Some(x.to_owned())),
                category: category.and_then(|x| Some(x.to_owned())),
                memo: memo.and_then(|x| Some(x.to_owned())),
                check: check.and_then(|x| Some(x.to_owned())),
            }),
        }
    }

    /// Check if the given header matches these rules.
    pub fn header_matches(&self, headers: &Vec<String>) -> bool {
        self.identify == *headers
    }

    /// Remap the columns in a mapping to what is desired on output.
    pub fn remap(&self, mut mapping: HashMap<String, String>) {
        // If this account does not define remappings we can just exist early.
        let Some(maps) = &self.translate else {
            return;
        };

        // Remap each column name if the remapping is defined.
        if let Some(k) = &maps.payee {
            if let Some(value) = mapping.remove(k) {
                mapping.insert("Payee".to_owned(), value);
            }
        }
        if let Some(k) = &maps.date {
            if let Some(value) = mapping.remove(k) {
                mapping.insert("Date".to_owned(), value);
            }
        }
        if let Some(k) = &maps.amount {
            if let Some(value) = mapping.remove(k) {
                mapping.insert("Amount".to_owned(), value);
            }
        }
        if let Some(k) = &maps.category {
            if let Some(value) = mapping.remove(k) {
                mapping.insert("Category".to_owned(), value);
            }
        }
        if let Some(k) = &maps.memo {
            if let Some(value) = mapping.remove(k) {
                mapping.insert("Memo".to_owned(), value);
            }
        }
        if let Some(k) = &maps.check {
            if let Some(value) = mapping.remove(k) {
                mapping.insert("Check#".to_owned(), value);
            }
        }
    }
}

/// Specification of how to remap CSV columns from the input to the output.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
struct RemapValuesCsv {
    /// The Payee column.
    payee: Option<String>,
    /// The Date column.
    date: Option<String>,
    /// The Amount column.
    amount: Option<String>,
    /// The Category column.
    category: Option<String>,
    /// The Memo column.
    memo: Option<String>,
    /// The Check# column.
    #[serde(rename = "Check#")]
    check: Option<String>,
}
