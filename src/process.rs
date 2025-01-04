use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Result};
use chrono::NaiveDate;

use crate::file_io::ensure_storage_path;
use crate::rules::MappingRulesCsv;
use crate::rules::RuleFileData;
use crate::{NormalizedBankData, TimestampKeeper};

/// Processing and storage of transaction data.
pub struct TransactionProcessor<'a> {
    /// The transactions to be written to disk.
    transactions: Vec<NormalizedBankData>,
    /// The mapping rules for this account type.
    mapping: &'a MappingRulesCsv,
    /// The rules for updating transactions.
    rules: &'a RuleFileData,
}

impl<'a> TransactionProcessor<'a> {
    /// Create a new instance.
    fn new(mapping: &'a MappingRulesCsv, rules: &'a RuleFileData) -> TransactionProcessor<'a> {
        TransactionProcessor {
            transactions: vec![],
            mapping,
            rules,
        }
    }

    /// Process a single transaction instance, possibly store the data.
    fn process(&mut self, data: HashMap<String, String>) -> Result<()> {
        // Convert the raw data into struct form.
        let mut norm = NormalizedBankData::from_raw_data(
            self.mapping.remap(data),
            self.mapping.negate,
            &self.mapping.date_fmt,
            &self.mapping.label,
        )?;

        // Update the contents of the transaction.
        self.rules.update_transaction(&mut norm);

        // Save the transaction.
        self.transactions.push(norm);
        Ok(())
    }

    /// Remove any transaction that should not remain according to rules.
    pub fn drop_uneeded(&mut self, start_date: &NaiveDate, end_date: &NaiveDate) {
        self.transactions
            .retain(|trans| !trans.skipme(start_date, end_date));
    }

    /// Return a string containing the CSV representation of the transactions.
    pub fn get_transactions_as_csv(&self) -> Result<String> {
        let mut wtr = csv::Writer::from_writer(vec![]);
        for transaction in self.transactions.iter() {
            wtr.serialize(transaction)?;
        }
        wtr.flush()?;
        Ok(String::from_utf8(wtr.into_inner()?)?)
    }
}

/// Given a list of CSV files, store each row as normalized and processed
/// data and return to the caller.
pub fn process_csv_files<'a>(
    paths: &Vec<impl AsRef<Path>>,
    rules: &'a RuleFileData,
) -> Result<HashMap<String, TransactionProcessor<'a>>> {
    let mut processors = HashMap::new();

    // Iterate over each CSV file.
    for path in paths {
        // Create the reader for this CSV file.
        let mut reader = csv::Reader::from_path(path.as_ref())?;

        // For this CSV, obtain the mapping rules for the account it represents.
        let mapping = rules
            .get_csv_mapping_rules(reader.headers()?)
            .ok_or_else(|| {
                anyhow!(format!(
                    "No rules are defined for the account corresponding to file {:#?}",
                    path.as_ref()
                ))
            })?;

        // If no processor has been created for this account type, create it now.
        if !processors.contains_key(&mapping.label) {
            processors.insert(
                mapping.label.to_owned(),
                TransactionProcessor::new(mapping, rules),
            );
        }

        // Obtain the processor for this transaction type.
        // .unwrap() is OK to use since we verified the label exists above.
        let processor = processors.get_mut(&mapping.label).unwrap();

        // For each row in this CSV, process and store the transactions.
        for row in reader.deserialize() {
            processor.process(row?)?;
        }
    }

    // Return the processors for the given CSV files.
    Ok(processors)
}

/// Account for the current timestamp in all transactions.
pub fn account_for_dates_in_transactions(
    now: &NaiveDate,
    all_transactions: &mut HashMap<String, TransactionProcessor>,
    stamps: &mut TimestampKeeper,
) {
    for (label, transactions) in all_transactions.iter_mut() {
        let start = stamps.get_date(label);
        transactions.drop_uneeded(&start, now);
        stamps.update_date(label, now);
    }
}

/// Write all transactions to the appropriate file.
pub fn write_transactions_to_file(
    now: impl AsRef<str>,
    storage: impl AsRef<Path>,
    all_transactions: &HashMap<String, TransactionProcessor>,
) -> Result<()> {
    // Write all transactions to file.
    let base = ensure_storage_path(storage, now, true)?;
    for (label, transactions) in all_transactions.iter() {
        let location = base.join(label.to_owned() + ".csv");
        fs::write(location, transactions.get_transactions_as_csv()?)?;
    }
    Ok(())
}
