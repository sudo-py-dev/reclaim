// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::SharedString;
use jiff::civil::Date;

pub fn use_24_hour_format() -> bool {
    true
}

fn civil_date(day: u32, month: u32, year: i32) -> Option<Date> {
    Date::new(year.try_into().ok()?, month.try_into().ok()?, day.try_into().ok()?).ok()
}

fn date_parts(date: Date) -> [i32; 3] {
    [date.day().into(), date.month().into(), date.year().into()]
}

/// Returns the number of days in a given month
pub fn month_day_count(month: u32, year: i32) -> Option<i32> {
    Some(civil_date(1, month, year)?.days_in_month().into())
}

pub fn month_offset(month: u32, year: i32) -> i32 {
    civil_date(1, month, year).map_or(0, |date| date.weekday().to_sunday_zero_offset().into())
}

pub fn format_date(format: &str, day: u32, month: u32, year: i32) -> SharedString {
    civil_date(day, month, year)
        .and_then(|date| jiff::fmt::strtime::format(format, date).ok())
        .map_or_else(SharedString::default, SharedString::from)
}

pub fn parse_date(date: &str, format: &str) -> Option<[i32; 3]> {
    Date::strptime(format, date).ok().map(date_parts)
}

#[cfg(feature = "std")]
pub fn date_now() -> [i32; 3] {
    date_parts(jiff::Zoned::now().date())
}

// display the today date is currently not implemented for no_std
#[cfg(not(feature = "std"))]
pub fn date_now() -> [i32; 3] {
    [-1, -1, -1]
}

#[cfg(feature = "ffi")]
mod ffi {
    #![expect(unsafe_code, reason = "The Slint C ABI requires stable exported symbols.")]

    use super::*;

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_date_time_use_24_hour_format() -> bool {
        use_24_hour_format()
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_date_time_month_day_count(month: u32, year: i32) -> i32 {
        month_day_count(month, year).unwrap_or(0)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_date_time_month_offset(month: u32, year: i32) -> i32 {
        month_offset(month, year)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_date_time_format_date(
        format: &SharedString,
        day: u32,
        month: u32,
        year: i32,
        out: &mut SharedString,
    ) {
        *out = format_date(format, day, month, year)
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_date_time_date_now(d: &mut i32, m: &mut i32, y: &mut i32) {
        [*d, *m, *y] = date_now();
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_date_time_parse_date(
        date: &SharedString,
        format: &SharedString,
        d: &mut i32,
        m: &mut i32,
        y: &mut i32,
    ) -> bool {
        if let Some(x) = parse_date(date, format) {
            [*d, *m, *y] = x;
            true
        } else {
            false
        }
    }
}
