use std::{collections::HashMap, fmt, marker::PhantomData, path, str::FromStr};

use anyhow::{anyhow, Result};
use expanduser::expanduser;
use rust_decimal::Decimal;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};

fn read_rules(raw_data: &str) -> Result<RuleFileData> {
    let rules: RuleFileData = toml::from_str(raw_data)?;

    // The storage directory must be a JSON file.
    if !rules.paths.timestamps.is_file()
        || rules
            .paths
            .timestamps
            .extension()
            .is_some_and(|x| x != "json")
    {
        return Err(anyhow!(format!(
            "The timestamps path {} is not a JSON file.",
            rules.paths.timestamps.to_str().unwrap()
        )));
    }

    // The storage directory must be a directory.
    if !rules.paths.storage.is_dir() {
        return Err(anyhow!(format!(
            "The storage path {} is not a directory.",
            rules.paths.storage.to_str().unwrap()
        )));
    }

    // Verify that each of the rules is unique.
    let mut check: HashMap<&PayeeRules, &String> = HashMap::new();
    for (payee, rules) in rules.payees.iter() {
        for rule in rules.iter() {
            if check.contains_key(&rule) {
                let other = check[rule];
                return Err(anyhow!(format!(
                    "The payees {:#?} and {:#?} both implement identical rules.",
                    other, payee
                )));
            }
            check.insert(rule, payee);
        }
    }

    // Return the results.
    Ok(rules)
}

/// The aggregation of all rules found in the rules file.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct RuleFileData {
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
    paths: AuxillaryPaths,
}

/// Holds rules for different types of input formats.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct MappingTypes {
    /// Rules for the CSV format.
    csv: Vec<MappingRulesCsv>,
}

/// Rules for specifying how to map a payee pattern to a specific payee.
/// The amount of the transaction can also be taken into account.
#[derive(Debug, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
struct PayeeRules {
    /// The pattern to use to identify a payee.
    pattern: String,
    /// The minimum range that a transaction must be for to identify as this payee.
    min_amount: Option<Decimal>,
    /// The maximum range that a transaction must be for to identify as this payee.
    max_amount: Option<Decimal>,
    /// The exact amount that a transaction must be for to identify as this payee.
    amount: Option<Decimal>,
}

/// Create this PayeeRules from a string.
impl FromStr for PayeeRules {
    // This implementation of `from_str` can never fail, so use the impossible
    // `Void` type as the error type.
    type Err = void::Void;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PayeeRules {
            pattern: s.to_string(),
            min_amount: None,
            max_amount: None,
            amount: None,
        })
    }
}

/// Rules for specifying how to identify a category or memo for
/// a given transaction.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "PascalCase")]
struct CategoryAndMemoRules {
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
    orig_payee: Option<String>,
}

/// Rules for how to identify CSV columns to accounts, and how
/// to map those column names to output column names.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct MappingRulesCsv {
    /// The label to give this account type.
    label: String,
    /// The columns to expect from this account type.
    identify: Vec<String>,
    /// How to map the given column names to output column names.
    translate: Option<RemapValuesCsv>,
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

/// Paths used by the program for various purposes.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct AuxillaryPaths {
    /// The path to the timestamps JSON file.
    #[serde(deserialize_with = "deserialize_path")]
    timestamps: path::PathBuf,
    /// The path to the directory where old and new CSV files will be stored.
    #[serde(deserialize_with = "deserialize_path")]
    storage: path::PathBuf,
}

/// Instructions on how to deserialize a path object.
pub fn deserialize_path<'de, D>(deserializer: D) -> Result<path::PathBuf, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    expanduser(s)
        .map_err(serde::de::Error::custom)?
        .canonicalize()
        .map_err(serde::de::Error::custom)
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
            E: de::Error,
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
            Deserialize::deserialize(de::value::MapAccessDeserializer::new(map))
        }
    }

    deserializer.deserialize_any(StringOrStruct(PhantomData))
}

/// Function to tell how to deserialize PayeeRules from either a map,
/// string, or vector of maps or strings.
fn hashmap_payee_rules<'de, D>(
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

/// Function to tell how to deserialize CategoryAndMemoRules from either a map,
/// or vector of maps.
fn hashmap_cat_memo_rules<'de, D>(
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
    use std::fs;

    use super::*;

    use indoc::indoc;
    use path::PathBuf;
    use rust_decimal_macros::dec;

    fn aux_paths(temp: &tempdir::TempDir) -> (PathBuf, PathBuf) {
        // Create the auxillary paths and their locations.
        let storage = temp.path().join("storage");
        let stamps = temp.path().join("timestamps.json");
        fs::create_dir(&storage).unwrap();
        fs::write(&stamps, "{}").unwrap();

        // Return the timestamps and storage location.
        (stamps, storage)
    }

    fn paths_section(stamps: &PathBuf, storage: &PathBuf) -> String {
        format!(
            indoc! { r#"
            [paths]
            timestamps = {:#?}
            storage = {:#?}
            "# },
            stamps, storage
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
        let (stamps, storage) = aux_paths(&temp);

        let given = indoc! { r#"
        [payees]
        Apple = "APPLE"
        Hulu = {Pattern = "PAYPAL INST TXFR", Amount = -24.00}
        Ace = [
            "ACE HARDWARE",
            {Pattern = "HARDWARE", MaxAmount = -20.00},
        ]

        [categories]
        Maintenance = {Payee = "The Home Depot", MinAmount = -50.00}
        Dining = [
            {Payee = "Subway"},
            {Payee = "Outback Steakhouse"},
        ]

        [memos]
        "Round-up" = {OrigPayee = "PNC", IncomeOK = true, Category = "Savings"}
        Parking = [
            {OrigPayee = "PARKING"},
            {Payee = "Johnson Garage"}
        ]

        [[mappings.csv]]
        label = "pnc"
        identify = ["Date", "Reference Number", "Payee", "Address", "Amount"]

        [[mappings.csv]]
        label = "ally"
        identify = ["Date", " Time", " Amount", " Type", " Description"]
        translate = {Amount = " Amount", Payee = " Description"}

        "# }
        .to_string()
            + &paths_section(&stamps, &storage);
        let expected = RuleFileData {
            payees: HashMap::from([
                (
                    "Apple".to_string(),
                    vec![PayeeRules::from_str("APPLE").unwrap()],
                ),
                (
                    "Hulu".to_string(),
                    vec![PayeeRules {
                        pattern: "PAYPAL INST TXFR".to_string(),
                        amount: Some(dec!(-24.00)),
                        min_amount: None,
                        max_amount: None,
                    }],
                ),
                (
                    "Ace".to_string(),
                    vec![
                        PayeeRules::from_str("ACE HARDWARE").unwrap(),
                        PayeeRules {
                            pattern: "HARDWARE".to_string(),
                            amount: None,
                            min_amount: None,
                            max_amount: Some(dec!(-20.00)),
                        },
                    ],
                ),
            ]),
            categories: Some(HashMap::from([
                (
                    "Maintenance".to_string(),
                    vec![CategoryAndMemoRules {
                        payee: Some("The Home Depot".to_string()),
                        amount: None,
                        min_amount: Some(dec!(-50.00)),
                        max_amount: None,
                        category: None,
                        income_ok: false,
                        orig_payee: None,
                    }],
                ),
                (
                    "Dining".to_string(),
                    vec![
                        CategoryAndMemoRules {
                            payee: Some("Subway".to_string()),
                            amount: None,
                            min_amount: None,
                            max_amount: None,
                            category: None,
                            income_ok: false,
                            orig_payee: None,
                        },
                        CategoryAndMemoRules {
                            payee: Some("Outback Steakhouse".to_string()),
                            amount: None,
                            min_amount: None,
                            max_amount: None,
                            category: None,
                            income_ok: false,
                            orig_payee: None,
                        },
                    ],
                ),
            ])),
            memos: Some(HashMap::from([
                (
                    "Round-up".to_string(),
                    vec![CategoryAndMemoRules {
                        payee: None,
                        amount: None,
                        min_amount: None,
                        max_amount: None,
                        category: Some("Savings".to_string()),
                        income_ok: true,
                        orig_payee: Some("PNC".to_string()),
                    }],
                ),
                (
                    "Parking".to_string(),
                    vec![
                        CategoryAndMemoRules {
                            payee: None,
                            amount: None,
                            min_amount: None,
                            max_amount: None,
                            category: None,
                            income_ok: false,
                            orig_payee: Some("PARKING".to_string()),
                        },
                        CategoryAndMemoRules {
                            payee: Some("Johnson Garage".to_string()),
                            amount: None,
                            min_amount: None,
                            max_amount: None,
                            category: None,
                            income_ok: false,
                            orig_payee: None,
                        },
                    ],
                ),
            ])),
            mappings: MappingTypes {
                csv: vec![
                    MappingRulesCsv {
                        label: "pnc".to_string(),
                        identify: ["Date", "Reference Number", "Payee", "Address", "Amount"]
                            .iter()
                            .map(|x| x.to_string())
                            .collect(),
                        translate: None,
                    },
                    MappingRulesCsv {
                        label: "ally".to_string(),
                        identify: ["Date", " Time", " Amount", " Type", " Description"]
                            .iter()
                            .map(|x| x.to_string())
                            .collect(),
                        translate: Some(RemapValuesCsv {
                            amount: Some(" Amount".to_string()),
                            payee: Some(" Description".to_string()),
                            date: None,
                            check: None,
                            category: None,
                            memo: None,
                        }),
                    },
                ],
            },
            paths: AuxillaryPaths {
                timestamps: stamps.canonicalize().unwrap(),
                storage: storage.canonicalize().unwrap(),
            },
        };
        let result = read_rules(&given).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_rule_file_without_memo_and_category() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let (stamps, storage) = aux_paths(&temp);

        let given = minimal_rules() + &paths_section(&stamps, &storage);
        let expected = RuleFileData {
            payees: HashMap::from([(
                "Apple".to_string(),
                vec![PayeeRules::from_str("APPLE").unwrap()],
            )]),
            categories: None,
            memos: None,
            mappings: MappingTypes {
                csv: vec![MappingRulesCsv {
                    label: "pnc".to_string(),
                    identify: ["Date", "Reference Number", "Payee", "Address", "Amount"]
                        .iter()
                        .map(|x| x.to_string())
                        .collect(),
                    translate: None,
                }],
            },
            paths: AuxillaryPaths {
                timestamps: stamps.canonicalize().unwrap(),
                storage: storage.canonicalize().unwrap(),
            },
        };
        let result = read_rules(&given).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_categories_requires_mapping() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let (stamps, storage) = aux_paths(&temp);

        let given = minimal_rules()
            + indoc! { r#"
        [categories]
        Dining = "Subway"
        "# } + &paths_section(&stamps, &storage);
        assert!(read_rules(&given)
            .err()
            .unwrap()
            .to_string()
            .contains("Dining = \"Subway\"\n"));
    }

    #[test]
    fn test_memos_requires_mapping() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let (stamps, storage) = aux_paths(&temp);

        let given = minimal_rules()
            + indoc! { r#"
        [memos]
        Sandwich = "Subway"
        "# } + &paths_section(&stamps, &storage);
        assert!(read_rules(&given)
            .err()
            .unwrap()
            .to_string()
            .contains("Sandwich = \"Subway\"\n"));
    }

    #[test]
    fn test_cannot_repeat_patterns() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let (stamps, storage) = aux_paths(&temp);

        let given = indoc! { r#"
        [payees]
        "Apple" = [
            {Pattern = "PAYPAL", MinAmount = -50.00},
            "APPLE",
        ]
        "Microsoft" = [
            "MICROSOFT",
            {Pattern = "PAYPAL", MinAmount = -50.00},
        ]

        [[mappings.csv]]
        label = "pnc"
        identify = ["Date", "Reference Number", "Payee", "Address", "Amount"]

        "# }.to_string() + &paths_section(&stamps, &storage);
        assert_eq!(
            r#"The payees "Apple" and "Microsoft" both implement identical rules."#,
            read_rules(&given).err().unwrap().to_string()
        );
    }

    #[test]
    fn test_timestamps_must_exist() {
        let temp = tempdir::TempDir::new("test").unwrap();

        let given = minimal_rules()
            + &format!(
                indoc! { r#"
        [paths]
        timestamps = "/does/not/exist/file.json"
        storage = {:#?}
        "# },
                temp.path()
            );
        assert!(read_rules(&given)
            .err()
            .unwrap()
            .to_string()
            .contains("No such file or directory"));
    }

    #[test]
    fn test_timestamps_must_be_a_file() {
        let temp = tempdir::TempDir::new("test").unwrap();

        let given = minimal_rules()
            + &format!(
                indoc! { r#"
        [paths]
        timestamps = {:#?}
        storage = {:#?}
        "# },
                temp.path(),
                temp.path(),
            );
        assert!(read_rules(&given)
            .err()
            .unwrap()
            .to_string()
            .contains("is not a JSON file"));
    }

    #[test]
    fn test_timestamps_must_have_a_json_extension() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let csv = temp.path().join("file.csv");
        fs::write(&csv, "column").unwrap();

        let given = minimal_rules()
            + &format!(
                indoc! { r#"
        [paths]
        timestamps = {:#?}
        storage = {:#?}
        "# },
                csv,
                temp.path(),
            );
        assert!(read_rules(&given)
            .err()
            .unwrap()
            .to_string()
            .contains("is not a JSON file"));
    }

    #[test]
    fn test_storage_must_exist() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let (stamps, _) = aux_paths(&temp);

        let given = minimal_rules()
            + &format!(
                indoc! { r#"
        [paths]
        timestamps = {:#?}
        storage = "/does/not/exist"
        "# },
                stamps,
            );
        assert!(read_rules(&given)
            .err()
            .unwrap()
            .to_string()
            .contains("No such file or directory"));
    }

    #[test]
    fn test_storage_must_be_a_directory() {
        let temp = tempdir::TempDir::new("test").unwrap();
        let (stamps, _) = aux_paths(&temp);

        let given = minimal_rules()
            + &format!(
                indoc! { r#"
        [paths]
        timestamps = {:#?}
        storage = {:#?}
        "# },
                stamps, stamps
            );
        assert!(read_rules(&given)
            .err()
            .unwrap()
            .to_string()
            .contains("is not a directory"));
    }
}
