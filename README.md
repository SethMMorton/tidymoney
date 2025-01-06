# tidymoney

Clean up raw data downloaded from banks.

`tidymoney` uses rules that you declare to transform entries in
CSV files downloaded from banks into a format that better lends itself
to storing in a financial accounting application. The following actions
are performed:

- The account to which a `CSV` file is attatched to is automatically
  detected.
- Payee, category, and memo can all be customized with rules that are
  based on the payee string, given category, transaction amount, and/or
  the transaction date.
- Entries are automatically filtered out if not within an expected date
  range.
- Column names are normalized.

All rules exist in a `rules.toml` file that exists in a configuration
location that is platform-dependent; run `tidymoney show-config` to see
the location of this file.

Run `tidymoney create-config` to create the file if you have not already,
and then use `tidymoney edit-config` to open it up in `$EDITOR` to edit
(you could also manually open it, that's obviously fine too).

After you have edited your rules, you can clean up your files with
`tidymoney run <your csv files>`. `tidymoney` will then clean up the CSV
files and place new, normalized files in a storage path determine by
the `rules.toml` file, and also move the old CSV files to an adjascent
location. You can then load the new CSV files into your financial
application.

## `rules.toml` format

The `rules.toml` file has three high-level sections:

- The transaction transformation rules (`[payees]`, `[categories]`, and `[memos]`)
- The account mapping and configuration rules (`[[mappings.csv]]`)
- Storage locations (`[paths]`)

### The `[payees]` section

This section can be used to update the payee field of a transaction to
match your preferences.

There are four fields you can use to match a transaction:

- `Payee` (required) - This is a regular expression that will match the
                       value in the payee column as downloaded from your
                       bank.
- `MinAmount` - An upper range for the dollar amount of the transaction.
- `MaxAmount` - A lower range for the dollar amount of the transaction.
- `Amount` - A specific dollar amount of the transaction.
- `MinDateInMonth` - A lower-bound date within the month for the transaction.
                     Useful to identify monthly transactions with generic names.
                     A single number from 1-31 (inclusive) is provided, and the
                     number will be clipped against the number of days in the
                     month.
- `MaxDateInMonth` - An upper-bound date within the month for the transaction.
                     See `MinDateInMonth` for use and allowed values.
- `MinDateInYear` - A lower-bound date within the year for the transaction.
                    Useful to identify yearly transactions with generic names.
                    A two-element list is provided where the first number is the
                    month number (1-12, inclusive) and the second number is the
                    day number (1-number of days in the month, inclusive).
- `MaxDateInYear` - An upper-bound date within the year for the transaction.
                    See `MinDateInYear` for use and allowed values.

All of the dollar amount fields (`MinAmount`, `MaxAmount`, and `Amount`)
should be given as positive numbers whether or not the transaction is
a debit or credit.

The pairs `MinDateInMonth`/`MaxDateInMonth` and `MinDateInYear`/`MaxDateInYear`
both support "wraparound" dates. If the "min" date is later than the "max"
date, then it assumes the range goes from the end of one month/year to the
beginning of the next.

If you only want to specify `Payee`, then a single string can be given
instead of a mapping.

The key will the name of the payee for transactions that match the
given rules.

Multiple rules for a single payee can be given in a list.

**Example:**

```toml
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
```

### The `[categories]` section

This section can be used categorize transactions should they follow specific rules.
This section is processed *after* `[payees]`, so the `Payee` column will be
updated with the rules from that section before processing this section.

The fields you can use to match a transaction are:

- `Payee` - An exact match for the value of the `Payee` column.
- `Category` - An exact match fot the value of the `Category` column
               (note that since `[categories]` has obviously not been processed
               yet, this would be the category as provided by the bank itself).
- `Amount` - A specific dollar amount of the transaction.
- `MinAmount` - A lower range for the dollar amount of the transaction.
- `MaxAmount` - An upper range for the dollar amount of the transaction.
- `IncomeOk` - Whether or not income (a credit) can be considered - the
               default is `true`
- `OrigPayee` - This is a regular expression that will match the
                value in the payee column *as downloaded from your bank*.

Unlike `[payees]`, there are no required nor default values, so a single
string is not allowed - you must always specify a mapping.

**Example:**

```toml
[categories]
Dining = [
    {Payee = "Subway"},
    {Payee = "Outback Steakhouse"},
]
Insurance = {Payee = "XYZ Insurance", MinAmount = 50.00, MaxAmount = 75.00}
Payment = {Payee = "Credit Card Payment"}
```

### The `[memos]` section

This section can be used to add memos to specific transactions should they
follow specific rules. The keys are identical to the `[categories]` section.

This is processed after `[categories]` so you can also match on the `Category`
that might have been added when processing the `[categories]` section.

**Example:**

```toml
[memos]
"Interest" = {OrigPayee = "Interest Income", Category = "Savings"}
Parking = [
    {OrigPayee = "PARKING"},
    {Payee = "Johnson Garage"}
]
```

### The `[[mappings.csv]]` secion

This section defines how `tidymoney` will identify and interpret the data
found in the raw CSV files from your bank(s). Each new `[[mappings.csv]]` section
represents an account `tidymoney` can handle.

The keys are as follows:

- `label` - The name of the account to which this mapping applies.
            This will be the name of the normalized CSV file that is created.
- `identify` - A list of all the columns in the raw CSV file as downloaded
               from your bank *in the order in which they appear*.
               This is used to correlate a CSV file to a given account.
- `translate` - Map column names as found in the raw CSV to column names
                required by the normalied format. See below for what names
                are expected. The key is the desired name, and the value
                is the name as it appears in the raw CSV file.
- `debit_is_positve` - A Boolean indicate whether or not your bank reports
                       debits with a positive or negative number.
                       The default is `false`.
- `date_fmt` - The format in which the date is represented by your bank.
               The default is `%Y-%m-%d`; see
               https://docs.rs/chrono/latest/chrono/format/strftime/index.html#specifiers
               for how to specify date formats.

The keys `label` and `identify` are *required*, all others are optional.

Here are the column names expected by `tidymoney` - if your bank does
not use these names for the corresponding column then use the `translate`
field to map the bank's name to the correct name.

- `Payee` (required) - To whom a transaction was for (sometimes called
                       "description")
- `Category` - A category into which the transaction may be placed.
- `Memo` - A note that can be attatched to the transaction.
- `Amount` - The amount of the transaction. Must be negative for debits
             and positive for credits (use `debit_is_positive`) if your
             bank reports this in the opposite manner.
- `Date` - The date of the transaction.
- `Check#` - A check number.

**Example:**

```toml
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
```

### The `[paths]` section

There is only one key in the `[paths]` section, and that is `storage`. This
is the location where `tidymoney` will put the old and new CSV files for you.

This location will have timestamped folders created inside it where the CSV
files are placed, and inside each timestamp folder will be an "old" and "new"
folder - the "old" folder will contain the CSV files as downloaded from your bank,
and the "new" folder will contain the normalized CSV files, one for each bank
account that was seen during processing.

**Example:**

```toml
[paths]
storage = "/path/to/storage/location"
```