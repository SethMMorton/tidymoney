use std::fs;

use anyhow::{anyhow, Result};
use clap::Parser;
use config_finder::ConfigDirs;

use tidymoney::{
    account_for_dates_in_transactions, normalize_path, process_csv_files, store_raw_transactions,
    timestamps_path, write_transactions_to_file, RuleFileData, TimestampKeeper, DATE_FORMAT,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    files: Vec<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Locate where the internal data exists on disk.
    let mut conf_dirs = ConfigDirs::empty();
    let mut conf_files = conf_dirs
        .add_platform_config_dir()
        .search("tidymoney", "rules", "toml");
    let rule_file = normalize_path(conf_files.next().ok_or(anyhow!("TODO"))?.path())?;
    // TODO ensure parent directory of rule_file exists, if not, create it.
    // TODO handle no rules file yet.
    let stamps_file = timestamps_path(&rule_file)?;

    // Get the internal data from disk.
    let rule_data = fs::read_to_string(&rule_file)?;
    let stamps_data = fs::read_to_string(&stamps_file)?;
    let rules = RuleFileData::new(&rule_data)?;
    let mut stamps = TimestampKeeper::new(&stamps_data)?;

    // Process the transactions.
    let mut results = process_csv_files(&cli.files, &rules)?;

    // Apply the current time to transactions and the timestamp records.
    let now = chrono::offset::Local::now().naive_local().date();
    account_for_dates_in_transactions(&now, &mut results, &mut stamps);

    // Write the new transactions to file.
    let now_str = now.format(DATE_FORMAT).to_string();
    write_transactions_to_file(&now_str, &rules.paths.storage, &results)?;

    // Write save the old files in the storage location.
    store_raw_transactions(&rules.paths.storage, &cli.files, &now_str)?;

    // Update the timestamps path.
    fs::write(&stamps_file, stamps.get_updated_stamps()?)?;

    Ok(())
}
