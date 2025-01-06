use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::Deserialize;

/// Rules for how to identify CSV columns to accounts, and how
/// to map those column names to output column names.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MappingRulesCsv {
    /// The label to give this account type.
    pub label: String,
    /// The columns to expect from this account type.
    identify: Vec<String>,
    /// How to map the given column names to output column names.
    translate: Option<RemapValuesCsv>,
    /// The format string for dates for this rule set.
    #[serde(default = "default_fmt_string")]
    pub date_fmt: String,
    /// Whether or not we need to negate the value of a transaction.
    #[serde(rename = "debit_is_positive", default)]
    pub negate: bool,
}

/// The default format string to use if not specified.
fn default_fmt_string() -> String {
    "%Y-%m-%d".to_owned()
}

impl MappingRulesCsv {
    /// Construct a new object - only needed for testing.
    #[cfg(test)]
    pub fn new(
        label: String,
        identify: Vec<String>,
        translate: HashMap<String, String>,
        date_fmt: Option<String>,
        negate: bool,
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
            date_fmt: date_fmt.unwrap_or(default_fmt_string()),
            negate,
        }
    }

    /// Check if the given header matches these rules.
    pub fn header_matches(&self, headers: &Vec<String>) -> bool {
        self.identify == *headers
    }

    /// Ensure all mapping keys appear in the identify vector.
    pub fn validate(&self) -> Result<()> {
        if let Some(trans) = &self.translate {
            let values = vec![
                &trans.payee,
                &trans.date,
                &trans.amount,
                &trans.category,
                &trans.memo,
                &trans.check,
            ];
            for value in values {
                if let Some(val) = &value {
                    if !self.identify.contains(val) {
                        return Err(anyhow!(
                            "The account {} lists {} for translation {}",
                            "but it is not listed in identify",
                            &self.label,
                            val,
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    /// Remap the columns in a mapping to what is desired on output.
    pub fn remap(&self, mut mapping: HashMap<String, String>) -> HashMap<String, String> {
        // If this account does not define remappings we can just exist early.
        let Some(maps) = &self.translate else {
            return mapping;
        };

        // Pair up each field with a key to which to map in the mapping.
        let pairs = vec![
            ("Payee", &maps.payee),
            ("Date", &maps.date),
            ("Amount", &maps.amount),
            ("Category", &maps.category),
            ("Memo", &maps.memo),
            ("Check#", &maps.check),
        ];

        // Remap each column name if the remapping is defined.
        for (key, value) in pairs {
            if let Some(k) = value {
                if let Some(val) = mapping.remove(k) {
                    mapping.insert(key.to_owned(), val);
                }
            }
        }

        mapping
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

#[cfg(test)]
mod test {
    use super::*;

    use pretty_assertions::assert_eq;
    use rstest::{fixture, rstest};

    use crate::as_hashmap;

    #[fixture]
    fn identify() -> Vec<String> {
        vec![
            "Transaction ID",
            "Posting Date",
            "Effective Date",
            "Transaction Type",
            "Amount",
            "Check Number",
            "Reference Number",
            "Description",
            "Transaction Category",
            "Type",
            "Balance",
            "Memo",
            "Extended Description",
        ]
        .into_iter()
        .map(|x| x.to_string())
        .collect()
    }

    #[rstest]
    #[case(vec!["hello"], false)]
    #[case(  // Missing transactions
        vec![
            "Transaction ID",
            "Posting Date",
            "Effective Date",
            "Balance",
            "Memo",
            "Extended Description",
        ],
        false,
    )]
    #[case(  // Incorrect order
        vec![
            "Transaction ID",
            "Posting Date",
            "Effective Date",
            "Transaction Type",
            "Check Number",
            "Amount",
            "Reference Number",
            "Description",
            "Transaction Category",
            "Type",
            "Balance",
            "Memo",
            "Extended Description",
        ],
        false,
    )]
    #[case(
        vec![
            "Transaction ID",
            "Posting Date",
            "Effective Date",
            "Transaction Type",
            "Amount",
            "Check Number",
            "Reference Number",
            "Description",
            "Transaction Category",
            "Type",
            "Balance",
            "Memo",
            "Extended Description",
        ],
        true,
    )]
    fn test_header_matches(
        #[case] given: Vec<&str>,
        #[case] expected: bool,
        identify: Vec<String>,
    ) {
        let label = "testing";
        let result = MappingRulesCsv::new(label.to_string(), identify, HashMap::new(), None, false)
            .header_matches(&given.into_iter().map(|x| x.to_string()).collect());
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(vec![], true)]
    #[case(vec![("payee", "Vendor")], false)]
    #[case(vec![("check", "Checking Number")], false)]
    #[case(vec![("date", "Some Date")], false)]
    #[case(vec![("amount", "Value")], false)]
    #[case(vec![("memo", "Note")], false)]
    #[case(vec![("category", "Column")], false)]
    #[case(
        vec![
            ("date", "Posting Date"),
            ("payee", "Description"),
            ("category", "Transaction Category"),
            ("check", "Check Number"),
        ],
        true,
    )]
    #[case(
        vec![
            ("memo", "Memo"),
            ("amount", "Amount"),
            ("date", "Posting Date"),
            ("payee", "Description"),
            ("category", "Transaction Category"),
            ("check", "Check Number"),
        ],
        true,
    )]
    fn test_validate(
        #[case] given: Vec<(&str, &str)>,
        #[case] expected: bool,
        identify: Vec<String>,
    ) {
        let label = "testing";
        let result =
            MappingRulesCsv::new(label.to_string(), identify, as_hashmap(given), None, false)
                .validate()
                .is_ok();
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(
        vec![("date", "Posting Date")],
        vec![
            ("Transaction ID", "12345"),
            ("Date", "2022-03-04"),
            ("Amount", "-14.53"),
            ("Check Number", "1234"),
            ("Description", "ACE"),
            ("Transaction Category", "Hardware"),
            ("Memo", "Things"),
        ]
    )]
    #[case(
        vec![
            ("date", "Posting Date"),
            ("check", "Check Number"),
            ("payee", "Description"),
            ("category", "Transaction Category"),
        ],
        vec![
            ("Transaction ID", "12345"),
            ("Date", "2022-03-04"),
            ("Amount", "-14.53"),
            ("Check#", "1234"),
            ("Payee", "ACE"),
            ("Category", "Hardware"),
            ("Memo", "Things"),
        ]
    )]
    fn test_remap(
        #[case] translate: Vec<(&str, &str)>,
        #[case] expected: Vec<(&str, &str)>,
        identify: Vec<String>,
    ) {
        let label = "testing";
        let mapping = vec![
            ("Transaction ID", "12345"),
            ("Posting Date", "2022-03-04"),
            ("Amount", "-14.53"),
            ("Check Number", "1234"),
            ("Description", "ACE"),
            ("Transaction Category", "Hardware"),
            ("Memo", "Things"),
        ];
        let obj = MappingRulesCsv::new(
            label.to_string(),
            identify,
            as_hashmap(translate),
            None,
            false,
        );
        assert_eq!(obj.remap(as_hashmap(mapping)), as_hashmap(expected));
    }
}
