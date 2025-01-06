mod category_and_memo;
mod date_filter;
mod eqregex;
mod mapping;
mod paths;
mod payees;

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::rules::category_and_memo::{hashmap_cat_memo_rules, CategoryAndMemoRules};
use crate::rules::paths::AuxillaryPaths;
use crate::rules::payees::{hashmap_payee_rules, PayeeRules};
use crate::NormalizedBankData;

pub use crate::rules::mapping::MappingRulesCsv;
pub use crate::rules::paths::normalize_path;

/// The aggregation of all rules found in the rules file.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RuleFileData {
    /// Rules to map a given payee name to the desired value.
    #[serde(deserialize_with = "hashmap_payee_rules")]
    payees: HashMap<String, Vec<PayeeRules>>,
    /// Rules to either map or synthesize a category for some transaction.
    #[serde(deserialize_with = "hashmap_cat_memo_rules", default)]
    categories: Option<HashMap<String, Vec<CategoryAndMemoRules>>>,
    /// Rules to synthesize a memo for some transaction.
    #[serde(deserialize_with = "hashmap_cat_memo_rules", default)]
    memos: Option<HashMap<String, Vec<CategoryAndMemoRules>>>,
    /// Rules for how to identify and translate files for different account types.
    mappings: MappingTypes,
    /// Locations of paths used by the program.
    pub paths: AuxillaryPaths,
}

impl RuleFileData {
    // Create a new RuleFileData from raw string data.
    pub fn new(raw_data: impl AsRef<str>) -> Result<Self> {
        let rules: RuleFileData = toml::from_str(raw_data.as_ref())?;
        rules.validate()?;
        Ok(rules)
    }

    /// Determine to which account the given headers correlate.
    pub fn get_csv_mapping_rules(&self, headers: &csv::StringRecord) -> Option<&MappingRulesCsv> {
        // Convert the headers object into a vector of strings so it can be compared.
        let hdrs: Vec<String> = headers.iter().map(|x| x.to_owned()).collect();

        // Identify the mapping rules that match the headers found.
        // If no rules were found, return None.
        self.mappings
            .csv
            .iter()
            .find(|&candidates| candidates.header_matches(&hdrs))
    }

    /// Run the transaction through the updating functions.
    pub fn update_transaction(&self, transaction: &mut NormalizedBankData) {
        self.update_payee(transaction);
        self.update_category(transaction);
        self.update_memo(transaction);
    }

    /// Determine a better payee name if available.
    fn update_payee(&self, transaction: &mut NormalizedBankData) {
        for (payee, candidates) in &self.payees {
            for candidate in candidates {
                if candidate.transaction_matches(transaction) {
                    transaction.payee = payee.to_owned();
                    break;
                }
            }
        }
    }

    /// Determine a better category if available.
    fn update_category(&self, transaction: &mut NormalizedBankData) {
        if let Some(cat) = &self.categories {
            for (category, candidates) in cat {
                for candidate in candidates {
                    if candidate.transaction_matches(transaction) {
                        transaction.category = Some(category.to_owned());
                        break;
                    }
                }
            }
        }
    }

    /// Determine a better memo if available.
    fn update_memo(&self, transaction: &mut NormalizedBankData) {
        if let Some(memos) = &self.memos {
            for (memo, candidates) in memos {
                for candidate in candidates {
                    if candidate.transaction_matches(transaction) {
                        transaction.memo = Some(memo.to_owned());
                        break;
                    }
                }
            }
        }
    }

    /// Ensure the read-in rules make logical sense.
    fn validate(&self) -> Result<()> {
        self.paths.validate()?;
        for mapping in &self.mappings.csv {
            mapping.validate()?;
        }

        // Verify that each of the rules is unique.
        #[allow(clippy::mutable_key_type)]
        let mut check: HashMap<&PayeeRules, &String> = HashMap::new();
        for (payee, rules) in self.payees.iter() {
            for rule in rules.iter() {
                if check.contains_key(&rule) {
                    let other = check[rule];
                    let values = if other < payee {
                        (other, payee)
                    } else {
                        (payee, other)
                    };
                    return Err(anyhow!(
                        "The payees {:#?} and {:#?} both implement identical rules.",
                        values.0,
                        values.1
                    ));
                }
                check.insert(rule, payee);
            }
        }

        // Verify the contents of the payee rules are correct.
        for (name, payees) in &self.payees {
            for payee in payees {
                payee.validate(name)?;
            }
        }

        // Verify that categories and memos have at least one check implemented.
        if let Some(c) = &self.categories {
            for (cat_name, categories) in c {
                for category in categories {
                    category.validate("category", cat_name)?;
                }
            }
        }
        if let Some(m) = &self.memos {
            for (memo_name, memos) in m {
                for memo in memos {
                    memo.validate("memo", memo_name)?;
                }
            }
        }

        Ok(())
    }
}

/// Holds rules for different types of input formats.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct MappingTypes {
    /// Rules for the CSV format.
    csv: Vec<MappingRulesCsv>,
}

#[cfg(test)]
mod test {
    use std::{fs, path::PathBuf, str::FromStr};

    use super::*;

    use indoc::indoc;
    use pretty_assertions::assert_eq;

    use crate::as_hashmap;

    fn aux_paths(temp: &tempdir::TempDir) -> PathBuf {
        let storage = temp.path().join("storage");
        fs::create_dir(&storage).unwrap();
        storage
    }

    fn paths_section(storage: &PathBuf) -> String {
        format!(
            indoc! { r#"
            [paths]
            storage = {:#?}
            "# },
            storage
        )
    }

    fn minimal_rules() -> String {
        indoc! { r#"
        [payees]
        Apple = "APPLE"

        [[mappings.csv]]
        label = "pnc"
        identify = ["Date", "Reference Number", "Payee", "Address", "Amount"]

        "# }
        .to_string()
    }

    #[test]
    fn test_rule_file_with_everything() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let storage = aux_paths(&temp);

        let given = indoc! { r#"
        [payees]
        Apple = "APPLE"
        Hulu = {Pattern = "PAYPAL INST TXFR", Amount = 24.00, MinDateInYear = [3, 6], MaxDateInYear = [5, 7]}
        Ace = [
            "ACE HARDWARE",
            {Pattern = "HARDWARE", MaxAmount = 20.00, MinDateInMonth = 4, MaxDateInMonth = 7},
        ]

        [categories]
        Maintenance = {Payee = "The Home Depot", MinAmount = 50.00}
        Dining = [
            {Payee = "Subway"},
            {Payee = "Outback Steakhouse"},
        ]

        [memos]
        "Round-up" = {OrigPayee = "PNC", Category = "Savings", MinDateInMonth = 20}
        Parking = [
            {OrigPayee = "PARKING", IncomeOK = false},
            {Payee = "Johnson Garage"}
        ]

        [[mappings.csv]]
        label = "pnc"
        identify = ["Date", "Reference Number", "Payee", "Address", "Amount"]
        date_fmt = "%Y/%m/%d"
        debit_is_positive = true

        [[mappings.csv]]
        label = "ally"
        identify = ["Date", " Time", " Amount", " Type", " Description"]
        translate = {Amount = " Amount", Payee = " Description"}

        "# }
        .to_string()
            + &paths_section(&storage);
        let expected = RuleFileData {
            payees: HashMap::from([
                (
                    "Apple".to_string(),
                    vec![PayeeRules::from_str("APPLE").unwrap()],
                ),
                (
                    "Hulu".to_string(),
                    vec![PayeeRules::new(as_hashmap(vec![
                        ("pattern", "PAYPAL INST TXFR"),
                        ("amount", "24.00"),
                        ("min_date_in_year", "3/6"),
                        ("max_date_in_year", "5/7"),
                    ]))],
                ),
                (
                    "Ace".to_string(),
                    vec![
                        PayeeRules::from_str("ACE HARDWARE").unwrap(),
                        PayeeRules::new(as_hashmap(vec![
                            ("pattern", "HARDWARE"),
                            ("max_amount", "20.00"),
                            ("min_date_in_month", "4"),
                            ("max_date_in_month", "7"),
                        ])),
                    ],
                ),
            ]),
            categories: Some(HashMap::from([
                (
                    "Maintenance".to_string(),
                    vec![CategoryAndMemoRules::new(as_hashmap(vec![
                        ("payee", "The Home Depot"),
                        ("min_amount", "50.00"),
                    ]))],
                ),
                (
                    "Dining".to_string(),
                    vec![
                        CategoryAndMemoRules::new(as_hashmap(vec![("payee", "Subway")])),
                        CategoryAndMemoRules::new(as_hashmap(vec![(
                            "payee",
                            "Outback Steakhouse",
                        )])),
                    ],
                ),
            ])),
            memos: Some(HashMap::from([
                (
                    "Round-up".to_string(),
                    vec![CategoryAndMemoRules::new(as_hashmap(vec![
                        ("category", "Savings"),
                        ("orig_payee", "PNC"),
                        ("min_date_in_month", "20"),
                    ]))],
                ),
                (
                    "Parking".to_string(),
                    vec![
                        CategoryAndMemoRules::new(as_hashmap(vec![
                            ("orig_payee", "PARKING"),
                            ("income_ok", "false"),
                        ])),
                        CategoryAndMemoRules::new(as_hashmap(vec![("payee", "Johnson Garage")])),
                    ],
                ),
            ])),
            mappings: MappingTypes {
                csv: vec![
                    MappingRulesCsv::new(
                        "pnc".to_string(),
                        ["Date", "Reference Number", "Payee", "Address", "Amount"]
                            .iter()
                            .map(|x| x.to_string())
                            .collect(),
                        [].into(),
                        Some("%Y/%m/%d".to_owned()),
                        true,
                    ),
                    MappingRulesCsv::new(
                        "ally".to_string(),
                        ["Date", " Time", " Amount", " Type", " Description"]
                            .iter()
                            .map(|x| x.to_string())
                            .collect(),
                        [("amount", " Amount"), ("payee", " Description")]
                            .iter()
                            .map(|(x, y)| (x.to_string(), y.to_string()))
                            .collect(),
                        None,
                        false,
                    ),
                ],
            },
            paths: AuxillaryPaths::new(storage),
        };
        let result = RuleFileData::new(&given).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_rule_file_without_memo_and_category() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let storage = aux_paths(&temp);

        let given = minimal_rules() + &paths_section(&storage);
        let expected = RuleFileData {
            payees: HashMap::from([(
                "Apple".to_string(),
                vec![PayeeRules::from_str("APPLE").unwrap()],
            )]),
            categories: None,
            memos: None,
            mappings: MappingTypes {
                csv: vec![MappingRulesCsv::new(
                    "pnc".to_string(),
                    ["Date", "Reference Number", "Payee", "Address", "Amount"]
                        .iter()
                        .map(|x| x.to_string())
                        .collect(),
                    [].into(),
                    None,
                    false,
                )],
            },
            paths: AuxillaryPaths::new(storage),
        };
        let result = RuleFileData::new(&given).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_categories_requires_mapping() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let storage = aux_paths(&temp);

        let given = minimal_rules()
            + indoc! { r#"
        [categories]
        Dining = "Subway"
        "# } + &paths_section(&storage);
        assert!(RuleFileData::new(&given)
            .err()
            .unwrap()
            .to_string()
            .contains("Dining = \"Subway\"\n"));
    }

    #[test]
    fn test_memos_requires_mapping() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let storage = aux_paths(&temp);

        let given = minimal_rules()
            + indoc! { r#"
        [memos]
        Sandwich = "Subway"
        "# } + &paths_section(&storage);
        assert!(RuleFileData::new(&given)
            .err()
            .unwrap()
            .to_string()
            .contains("Sandwich = \"Subway\"\n"));
    }

    #[test]
    fn test_cannot_repeat_patterns() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let storage = aux_paths(&temp);

        let given = indoc! { r#"
        [payees]
        "Apple" = [
            {Pattern = "PAYPAL", MinAmount = 50.00},
            "APPLE",
        ]
        "Microsoft" = [
            "MICROSOFT",
            {Pattern = "PAYPAL", MinAmount = 50.00},
        ]

        [[mappings.csv]]
        label = "pnc"
        identify = ["Date", "Reference Number", "Payee", "Address", "Amount"]

        "# }
        .to_string()
            + &paths_section(&storage);
        assert_eq!(
            r#"The payees "Apple" and "Microsoft" both implement identical rules."#,
            RuleFileData::new(&given).err().unwrap().to_string()
        );
    }
}
