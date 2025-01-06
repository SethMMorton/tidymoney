use std::cmp::min;

use anyhow::{anyhow, Result};
use chrono::{Datelike, NaiveDate};

#[cfg(test)]
use std::collections::HashMap;

type MonthFilters = (Option<u32>, Option<u32>);
type YearFilters = (Option<(u32, u32)>, Option<(u32, u32)>);

/// Assess if the date is outside the range.
pub fn date_is_outside_range(date: &impl Datelike, month: MonthFilters, year: YearFilters) -> bool {
    date_is_ouside_range_in_month(date, month) || date_is_ouside_range_in_year(date, year)
}

/// Assess if the date is outside the in-month range.
fn date_is_ouside_range_in_month(date: &impl Datelike, month_filters: MonthFilters) -> bool {
    // Extract the date within the month from the date object.
    let date_in_month = date.day();

    // Get the last possible date in this month - we will clip
    // the high range to this value.
    let last_month_date = last_date_in_month(date);

    // The check depends what is defined...
    match month_filters {
        // If both high and low are provided, then the value
        // cannot be outside either bounds.
        // Note that we are allowing "wraparound" bounds, meaning
        // that we might give the 20th of the month to the 5th of
        // the next month, in which case high will be less than
        // low and outside the range is greater than low and less
        // than high.
        (Some(low), Some(high)) => {
            let low = min(low, last_month_date);
            let high = min(high, last_month_date);
            if high < low {
                !(date_in_month <= high || date_in_month >= low)
            } else {
                !(date_in_month >= low && date_in_month <= high)
            }
        }
        // If only one end of the range is defined, simply check if the date
        // is on the wrong side of that value.
        (Some(low), None) => date_in_month < min(low, last_month_date),
        (None, Some(high)) => date_in_month > min(high, last_month_date),
        // If neither high nor low are defined, then there is no range to be outside.
        (None, None) => false,
    }
}

/// Assess if the date is outside the in-year range.
fn date_is_ouside_range_in_year(date: &impl Datelike, year_filters: YearFilters) -> bool {
    // Extract the date within the month from the date object.
    let date_in_year = (date.month(), date.day());

    // The check depends what is defined...
    match year_filters {
        // If both high and low are provided, then the value
        // cannot be outside either bounds.
        // Note that we are allowing "wraparound" bounds, meaning
        // that we might give the a December date to a January
        // date, in which case high will be less than low and outside
        // the range is greater than low and less than high.
        (Some(low), Some(high)) => {
            if high < low {
                !(date_in_year <= high || date_in_year >= low)
            } else {
                !(date_in_year >= low && date_in_year <= high)
            }
        }
        // If only one end of the range is defined, simply check if the date
        // is on the wrong side of that value.
        (Some(low), None) => date_in_year < low,
        (None, Some(high)) => date_in_year > high,
        // If neither high nor low are defined, then there is no range to be outside.
        (None, None) => false,
    }
}

// Return the last date in the month - account for leap years.
fn last_date_in_month(date: &impl Datelike) -> u32 {
    let month = date.month();
    let is_leap = NaiveDate::from_yo_opt(date.year(), 1).unwrap().leap_year();
    if month == 2 && is_leap {
        29
    } else {
        last_date_in_month_raw(month)
    }
}

// Return the last date in the month, based only on the month number.
fn last_date_in_month_raw(month: u32) -> u32 {
    match month {
        2 => 28,
        4 => 30,
        6 => 30,
        9 => 30,
        11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
/// Parse date filter date - for testing only.
pub fn process_date_filter_mapping(
    mapping: HashMap<String, String>,
) -> (
    Option<u32>,
    Option<u32>,
    Option<(u32, u32)>,
    Option<(u32, u32)>,
) {
    let min_date_in_month = mapping
        .get("min_date_in_month")
        .and_then(|x| x.parse().ok());
    let max_date_in_month = mapping
        .get("max_date_in_month")
        .and_then(|x| x.parse().ok());
    let min_date_in_year = mapping
        .get("min_date_in_year")
        .and_then(|x| Some(x.split('/').collect::<Vec<_>>()))
        .map(|x| {
            (
                x.get(0).unwrap().parse().unwrap(),
                x.get(1).unwrap().parse().unwrap(),
            )
        });
    let max_date_in_year = mapping
        .get("max_date_in_year")
        .and_then(|x| Some(x.split('/').collect::<Vec<_>>()))
        .map(|x| {
            (
                x.get(0).unwrap().parse().unwrap(),
                x.get(1).unwrap().parse().unwrap(),
            )
        });
    (
        min_date_in_month,
        max_date_in_month,
        min_date_in_year,
        max_date_in_year,
    )
}

/// Ensure the given rules are semantically correct.
pub fn validate_date_filters(
    obj_type: &str,
    name: &str,
    month: MonthFilters,
    year: YearFilters,
) -> Result<()> {
    let (min_date_in_month, max_date_in_month) = month;
    let (min_date_in_year, max_date_in_year) = year;
    if min_date_in_month.is_some_and(|x| !(1..=31).contains(&x)) {
        return Err(anyhow!(
            "The {obj_type} {name:#?} specifies a MinDateInMonth that is not in [1, 31]."
        ));
    }
    if max_date_in_month.is_some_and(|x| !(1..=31).contains(&x)) {
        return Err(anyhow!(
            "The {obj_type} {name:#?} specifies a MaxDateInMonth that is not in [1, 31]."
        ));
    }
    if let Some((low_month, low_day)) = min_date_in_year {
        if !(1..=31).contains(&low_day) || !(1..=12).contains(&low_month) {
            return Err(anyhow!(
                "The {obj_type} {name:#?} specifies a MinDateInYear where the month {}",
                "is not in [1, 12] or the day is not in [1, 31]."
            ));
        }
        let last = last_date_in_month_raw(low_month);
        if low_day > last {
            return Err(anyhow!(
                "The {obj_type} {name:#?} {} ({low_day}) {} ({last})",
                "specifies a MinDateInYear where the given date",
                "is greater than the number of days in that month",
            ));
        }
    }
    if let Some((high_month, high_day)) = max_date_in_year {
        if !(1..=31).contains(&high_day) || !(1..=12).contains(&high_month) {
            return Err(anyhow!(
                "The {obj_type} {name:#?} specifies a MaxDateInYear where the month {}",
                "is not in [1, 12] or the day is not in [1, 31]."
            ));
        }
        let last = last_date_in_month_raw(high_month);
        if high_day > last {
            return Err(anyhow!(
                "The {obj_type} {name:#?} {} ({high_day}) {} ({last})",
                "specifies a MaxDateInYear where the given date",
                "is greater than the number of days in that month",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    use rstest::rstest;

    #[rstest]
    #[case(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(), 31)]
    #[case(NaiveDate::from_ymd_opt(2024, 2, 1).unwrap(), 29)]
    #[case(NaiveDate::from_ymd_opt(2023, 2, 1).unwrap(), 28)]
    #[case(NaiveDate::from_ymd_opt(2024, 3, 1).unwrap(), 31)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 1).unwrap(), 30)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 1).unwrap(), 31)]
    #[case(NaiveDate::from_ymd_opt(2024, 6, 1).unwrap(), 30)]
    #[case(NaiveDate::from_ymd_opt(2024, 7, 1).unwrap(), 31)]
    #[case(NaiveDate::from_ymd_opt(2024, 8, 1).unwrap(), 31)]
    #[case(NaiveDate::from_ymd_opt(2024, 9, 1).unwrap(), 30)]
    #[case(NaiveDate::from_ymd_opt(2024, 10, 1).unwrap(), 31)]
    #[case(NaiveDate::from_ymd_opt(2024, 11, 1).unwrap(), 30)]
    #[case(NaiveDate::from_ymd_opt(2024, 12, 1).unwrap(), 31)]
    fn test_last_date_in_month(#[case] given: NaiveDate, #[case] expected: u32) {
        let result = last_date_in_month(&given);
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(), None, None, false)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 1).unwrap(), None, None, false)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 16).unwrap(), None, None, false)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 2).unwrap(), Some(4), None, true)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 16).unwrap(), Some(4), None, false)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 26).unwrap(), None, Some(23), true)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 16).unwrap(), None, Some(23), false)]
    #[case(NaiveDate::from_ymd_opt(2024, 2, 29).unwrap(), None, Some(31), false)]
    #[case(NaiveDate::from_ymd_opt(2022, 2, 28).unwrap(), None, Some(31), false)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 30).unwrap(), None, Some(31), false)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 31).unwrap(), None, Some(31), false)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 9).unwrap(), Some(7), Some(15), false)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 3).unwrap(), Some(7), Some(15), true)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 20).unwrap(), Some(7), Some(15), true)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 31).unwrap(), Some(7), Some(15), true)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 9).unwrap(), Some(15), Some(7), true)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 3).unwrap(), Some(15), Some(7), false)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 20).unwrap(), Some(15), Some(7), false)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 31).unwrap(), Some(15), Some(7), false)]
    fn test_date_is_ouside_range_in_month(
        #[case] given: NaiveDate,
        #[case] low: Option<u32>,
        #[case] high: Option<u32>,
        #[case] expected: bool,
    ) {
        let result = date_is_ouside_range_in_month(&given, (low, high));
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(), None, None, false)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 1).unwrap(), None, None, false)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 16).unwrap(), None, None, false)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 2).unwrap(), Some((6, 4)), None, true)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 2).unwrap(), Some((2, 4)), None, false)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 2).unwrap(), None, Some((6, 4)), false)]
    #[case(NaiveDate::from_ymd_opt(2024, 4, 2).unwrap(), None, Some((2, 4)), true)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 9).unwrap(), Some((2, 7)), Some((6, 15)), false)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 9).unwrap(), Some((6, 15)), Some((8, 12)), true)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 9).unwrap(), Some((12, 2)), Some((2, 12)), true)]
    #[case(NaiveDate::from_ymd_opt(2024, 5, 9).unwrap(), Some((12, 2)), Some((5, 12)), false)]
    fn test_date_is_ouside_range_in_year(
        #[case] given: NaiveDate,
        #[case] low: Option<(u32, u32)>,
        #[case] high: Option<(u32, u32)>,
        #[case] expected: bool,
    ) {
        let result = date_is_ouside_range_in_year(&given, (low, high));
        assert_eq!(result, expected);
    }

    #[rstest]
    #[case(
        ((Some(0), None), (None, None)),
        "MinDateInMonth that is not in [1, 31]"
    )]
    #[case(
        ((Some(32), None), (None, None)),
        "MinDateInMonth that is not in [1, 31]"
    )]
    #[case(
        ((None, Some(0)), (None, None)),
        "MaxDateInMonth that is not in [1, 31]"
    )]
    #[case(
        ((None, Some(32)), (None, None)),
        "MaxDateInMonth that is not in [1, 31]"
    )]
    #[case(
        ((None, None), (Some((0, 1)), None)),
        "MinDateInYear where the month is not in [1, 12] or the day is not in [1, 31]"
    )]
    #[case(
        ((None, None), (Some((13, 1)), None)),
        "MinDateInYear where the month is not in [1, 12] or the day is not in [1, 31]"
    )]
    #[case(
        ((None, None), (Some((1, 0)), None)),
        "MinDateInYear where the month is not in [1, 12] or the day is not in [1, 31]"
    )]
    #[case(
        ((None, None), (Some((1, 32)), None)),
        "MinDateInYear where the month is not in [1, 12] or the day is not in [1, 31]"
    )]
    #[case(
        ((None, None), (Some((4, 31)), None)),
        "MinDateInYear where the given date (31) is greater than the number of days in that month (30)"
    )]
    #[case(
        ((None, None), (None, Some((0, 1)))),
        "MaxDateInYear where the month is not in [1, 12] or the day is not in [1, 31]"
    )]
    #[case(
        ((None, None), (None, Some((13, 1)))),
        "MaxDateInYear where the month is not in [1, 12] or the day is not in [1, 31]"
    )]
    #[case(
        ((None, None), (None, Some((1, 0)))),
        "MaxDateInYear where the month is not in [1, 12] or the day is not in [1, 31]"
    )]
    #[case(
        ((None, None), (None, Some((1, 32)))),
        "MaxDateInYear where the month is not in [1, 12] or the day is not in [1, 31]"
    )]
    #[case(
        ((None, None), (None, Some((4, 31)))),
        "MaxDateInYear where the given date (31) is greater than the number of days in that month (30)"
    )]

    fn test_validate(#[case] given: (MonthFilters, YearFilters), #[case] expected: &str) {
        let result = validate_date_filters("test", "test", given.0, given.1).unwrap_err();
        assert!(result.to_string().contains(expected));
    }
}
