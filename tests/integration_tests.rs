use std::path::Path;
use std::{collections::HashMap, fs};

use chrono::NaiveDate;
use indoc::indoc;
use pretty_assertions::assert_eq;
use rstest::{fixture, rstest};

use tidymoney::{
    account_for_dates_in_transactions, as_hashmap, process_csv_files, write_transactions_to_file,
    RuleFileData, TimestampKeeper, DATE_FORMAT,
};

#[rstest]
fn test_full_integration(
    sample_timestamps: String,
    sample_csv_files: Vec<String>,
    expected_results: HashMap<String, String>,
) {
    let temp = tempdir::TempDir::new("test").unwrap();
    let rule_file = sample_rule_file(&temp);
    let stamps_file = sample_timestamps;

    // Create sample CSV files.
    let mut sample_csv = vec![];
    for (i, data) in sample_csv_files.iter().enumerate() {
        sample_csv.push(temp.path().join(format!("{i}.csv")));
        fs::write(sample_csv.last().unwrap(), data).unwrap();
    }

    // Process the transaction data according to the rules from the rules file.
    let rules = RuleFileData::new(rule_file).unwrap();
    let mut stamps = TimestampKeeper::new(stamps_file).unwrap();
    let mut processed = process_csv_files(&sample_csv, &rules).unwrap();

    // Filter transactions according to the dates from the timestamps file.
    let now = NaiveDate::from_ymd_opt(2024, 10, 25).unwrap();
    account_for_dates_in_transactions(&now, &mut processed, &mut stamps);

    // Write the new transactions to file
    let now = now.format(DATE_FORMAT).to_string();
    write_transactions_to_file(&now, &temp, &processed).unwrap();

    // Ensure the written transactions appear as expected.
    let created_files = glob::glob(
        temp.path()
            .join("new")
            .join(now)
            .join("*.csv")
            .as_os_str()
            .to_str()
            .unwrap(),
    )
    .unwrap();
    let found_results = as_hashmap(
        created_files
            .map(|x| x.unwrap())
            .map(|x| {
                (
                    x.file_name().unwrap().to_str().unwrap().to_owned(),
                    fs::read_to_string(x).unwrap(),
                )
            })
            .collect(),
    );
    assert_eq!(found_results, expected_results);

    // Ensure that the timestamps are correctly updated.
    let new_stamps_str = indoc! {
        r#"
        [
            {"account": "discover", "date": "2024-10-25"},
            {"account": "bank_of_america", "date": "2024-10-25"},
            {"account": "ally", "date": "2024-10-25"}
        ]
        "#
    };
    let new_stamps = TimestampKeeper::new(new_stamps_str).unwrap();
    assert_eq!(stamps, new_stamps);
}

fn sample_rule_file(tempdir: impl AsRef<Path>) -> String {
    let transactions = tempdir.as_ref().join("transactions");
    fs::create_dir(&transactions).unwrap();
    let paths_section = format!(
        indoc! { r#"
        [paths]
        storage = {:#?}
        "# },
        &transactions
    );

    indoc! { r#"
    [payees]
    Ace = [
        "ACE HARDWARE",
        {Pattern = "HARDWARE", MaxAmount = 20.00},
    ]
    "Amazon.com" = [
        'AMAZON\.COM',
        "AMAZON MKTPL",
    ]
    Apple = "APPLE"
    "Credit Card Payment" = [
        "BA ELECTRONIC PAYMENT",
        "DIRECTPAY",
    ]
    Hulu = {Pattern = "PAYPAL INST XFER", Amount = 24.00}
    "Local Public Transit" = "LIGHT RAIL"
    Netflix = 'Netflix\.com'
    "The New York Times" = {Pattern = "PAYPAL INST XFER", Amount = 28.00}
    Salary = [
        "DIRDEP",
    ]
    Subway = [
        "Subway",
        "SUBWAY",
    ]
    Transfer = "Surprise Savings Booster Transfer to Savings Account"
    Visible = {Pattern = "PAYPAL INST XFER", Amount = 35.00}
    "XYZ Insurance" = {Pattern = "PAYPAL INST XFER", MinAmount = 65.00, MaxAmount = 75.00, MinDateInMonth = 15}

    [categories]
    Dining = [
        {Payee = "Subway"},
        {Payee = "Outback Steakhouse"},
    ]
    Insurance = {Payee = "XYZ Insurance"}
    "Net Income" = {Payee = "Salary"}
    Payment = {Payee = "Credit Card Payment"}
    Savings = {Payee = "Transfer"}
    Travel = {Payee = "Local Public Transit"}

    [memos]
    "Surprise!" = {OrigPayee = "Surprise", Category = "Savings"}
    Parking = [
        {OrigPayee = "PARKING"},
        {Payee = "Johnson Garage"}
    ]

    [[mappings.csv]]
    label = "bank_of_america"
    identify = ["Posted Date", "Reference Number", "Payee", "Address", "Amount"]
    translate = {Date = "Posted Date"}
    date_fmt = "%m/%d/%Y"

    [[mappings.csv]]
    label = "ally"
    identify = ["Date", " Time", " Amount", " Type", " Description"]
    translate = {Amount = " Amount", Payee = " Description"}

    [[mappings.csv]]
    label = "discover"
    identify = ["Trans. Date", "Post Date", "Description", "Amount", "Category"]
    translate = {Date = "Trans. Date", Payee = "Description"}
    debit_is_positive = true
    date_fmt = "%m/%d/%Y"

    "# }
    .to_string()
        + &paths_section
}

#[fixture]
fn sample_timestamps() -> String {
    indoc! { r#"
    [
        {
            "account": "discover",
            "date": "2023-03-15"
        },
        {
            "account": "ally",
            "date": "2024-01-04"
        }
    ]
    "#}
    .to_string()
}

#[fixture]
fn sample_csv_files() -> Vec<String> {
    vec![
        indoc!{ r#"
            Trans. Date,Post Date,Description,Amount,Category
            09/14/2024,09/14/2024,"AMAZON.COM*1234567",29.99,"Merchandise"
            09/13/2024,09/13/2024,"DIRECTPAY FULL BALANCESEE DETAILS OF YOUR NEXT DIRECTPAY BELOW",-616.62,"Payments and Credits"
            09/05/2024,09/05/2024,"PAYPAL INST XFER",35.00,"Services"
            08/31/2024,08/31/2024,"AMAZON MKTPL*1234567",24.99,"Merchandise"
        "# },
        indoc!{ r#"
            Trans. Date,Post Date,Description,Amount,Category
            10/22/2024,10/22/2024,"LIGHT RAIL FASTPASS",25.00,"Travel/ Entertainment"
        "# },
        indoc!{ r#"
            Trans. Date,Post Date,Description,Amount,Category
            04/03/2022,10/22/2024,"BARNS AND NOBLE",64.00,"Merchandise"
        "# },
        indoc!{ r#"
            Date, Time, Amount, Type, Description
            2024-10-26,23:37:23,-12.54,Withdrawal,Wendy's
            2024-10-23,23:37:23,0.14,Deposit,Interest Paid
            2024-10-23,15:31:30,-49.00,Withdrawal,Surprise Savings Booster Transfer to Savings Account
            2024-10-21,01:13:22,-15.99,Withdrawal,PAYPAL INST XFER
            2024-10-18,01:04:46,-69.75,Withdrawal,PAYPAL INST XFER
            2024-10-11,16:14:48,550.00,Deposit,ABC INC DIRDEP
            2024-10-03,01:04:46,-69.75,Withdrawal,PAYPAL INST XFER
            2024-09-28,13:52:23,0.00,Deposit,Ping
            2024-09-23,23:43:32,0.12,Deposit,Interest Paid
        "# },
        indoc!{ r#"
            Posted Date,Reference Number,Payee,Address,Amount
            09/26/2024,123456,"PP*APPLE.COM/BILL","402-935-7733  CA ",-7.99
            09/26/2024,123456,"PAYPAL INST XFER","402-935-7733  IL ",-14.29
            09/25/2024,123456,"PP*APPLE.COM/BILL","402-935-7733  CA ",-2.99
            09/24/2024,123456,"BA ELECTRONIC PAYMENT","",860.31
            09/24/2024,123456,"PAYPAL INST XFER","402-935-7733  NY ",-28.00
        "# },
        indoc!{ r#"
            Posted Date,Reference Number,Payee,Address,Amount
            10/24/2024,123456,"BA ELECTRONIC PAYMENT","",25.27
            10/18/2024,123456,"Netflix.com","866-5797172   CA ",-15.49
            10/14/2024,123456,"Subway 26689 Vancouver WA","Vancouver     WA ",-6.98
            10/14/2024,123456,"Subway 26689 Vancouver WA","Vancouver     WA ",-21.57
        "# },
    ].iter().map(|x| x.to_string()).collect()
}

#[fixture]
fn expected_results() -> HashMap<String, String> {
    as_hashmap(vec![
        (
            "ally.csv",
            indoc! {
                r#"
                Date,Payee,Category,Memo,Amount,Check#
                2024-10-23,Interest Paid,,,0.14,
                2024-10-23,Transfer,Savings,Surprise!,-49.00,
                2024-10-21,PAYPAL INST XFER,,,-15.99,
                2024-10-18,XYZ Insurance,Insurance,,-69.75,
                2024-10-11,Salary,Net Income,,550.00,
                2024-10-03,PAYPAL INST XFER,,,-69.75,
                2024-09-23,Interest Paid,,,0.12,
                "#
            },
        ),
        (
            "bank_of_america.csv",
            indoc! {
                r#"
                Date,Payee,Category,Memo,Amount,Check#
                2024-09-26,Apple,,,-7.99,
                2024-09-26,PAYPAL INST XFER,,,-14.29,
                2024-09-25,Apple,,,-2.99,
                2024-09-24,Credit Card Payment,Payment,,860.31,
                2024-09-24,The New York Times,,,-28.00,
                2024-10-24,Credit Card Payment,Payment,,25.27,
                2024-10-18,Netflix,,,-15.49,
                2024-10-14,Subway,Dining,,-6.98,
                2024-10-14,Subway,Dining,,-21.57,
                "#
            },
        ),
        (
            "discover.csv",
            indoc! {
                r#"
                Date,Payee,Category,Memo,Amount,Check#
                2024-09-14,Amazon.com,Merchandise,,-29.99,
                2024-09-13,Credit Card Payment,Payment,,616.62,
                2024-09-05,Visible,Services,,-35.00,
                2024-08-31,Amazon.com,Merchandise,,-24.99,
                2024-10-22,Local Public Transit,Travel,,-25.00,
                "#
            },
        ),
    ])
}
