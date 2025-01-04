use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use config_finder::ConfigDirs;
use indoc::indoc;

use tidymoney::{
    account_for_dates_in_transactions, normalize_path, process_csv_files, store_raw_transactions,
    timestamps_path, write_transactions_to_file, RuleFileData, TimestampKeeper, DATE_FORMAT,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "Run the tidymoney logic")]
    Run { files: Vec<String> },
    #[command(about = "Show the location of the rules.toml file")]
    ShowConfig {},
    #[command(about = "Create the rules.toml file")]
    CreateConfig {},
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::CreateConfig {} => {
            let rule_file = get_rule_file()?;
            if rule_file.is_file() {
                return Err(anyhow!(format!(
                    "The rule file {:#?} already exists.",
                    rule_file
                )));
            }
            let parent = rule_file
                .parent()
                .ok_or(anyhow!("Cannot identify parent of {:#?}", rule_file))?;
            if !parent.is_dir() {
                fs::create_dir_all(parent)?;
            }
            fs::write(
                rule_file,
                indoc! {
                    r#"
                [payees]
                Description = "RULE FOR NAMING TRANSACTION"

                [categories]

                [memos]

                [[mappings.csv]]
                label = "account_name"
                identify = ["Column1", "Column2", "Column3", "Column4"]
                # translate = {Date = "Posted Date"}  # Needed to map given columns to required columns

                [paths]
                storage = "/path/to/storage/directory"
                "#
                },
            )?;
            println!("Created {:#?}.\n", get_rule_file()?);
            println!("Edit this file to meet your needs.\n");
            println!("See URL for instructions.");
        }
        Commands::ShowConfig {} => {
            println!("{}", get_rule_file()?.to_str().unwrap());
        }
        Commands::Run { files } => {
            let rule_file = get_rule_file()?;
            if !rule_file.is_file() {
                return Err(anyhow!(format!(
                    "The file {:#?} does not exist - create it with 'tidymoney create-config'.",
                    rule_file
                )));
            }
            let stamps_file = timestamps_path(&rule_file)?;

            // Get the internal data from disk.
            let rule_data = fs::read_to_string(&rule_file)?;
            let stamps_data = fs::read_to_string(&stamps_file)?;
            let rules = RuleFileData::new(&rule_data)?;
            let mut stamps = TimestampKeeper::new(&stamps_data)?;

            // Process the transactions.
            let mut results = process_csv_files(&files, &rules)?;

            // Apply the current time to transactions and the timestamp records.
            let now = chrono::offset::Local::now().naive_local().date();
            account_for_dates_in_transactions(&now, &mut results, &mut stamps);

            // Write the new transactions to file.
            let now_str = now.format(DATE_FORMAT).to_string();
            write_transactions_to_file(&now_str, &rules.paths.storage, &results)?;

            // Write save the old files in the storage location.
            store_raw_transactions(&rules.paths.storage, &files, &now_str)?;

            // Update the timestamps path.
            fs::write(&stamps_file, stamps.get_updated_stamps()?)?;
        }
    }

    Ok(())
}

/// Return the path to the rules.toml file.
fn get_rule_file() -> Result<PathBuf> {
    let mut conf_dirs = ConfigDirs::empty();
    let mut conf_files = conf_dirs
        .add_platform_config_dir()
        .search("tidymoney", "rules", "toml");
    normalize_path(
        conf_files
            .next()
            .ok_or(anyhow!("Cannot identify the path to the rules.toml file"))?
            .path(),
    )
}
