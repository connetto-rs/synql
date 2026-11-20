//! Submodule reading the text of a PostgreSQL interval into microseconds.
//!
//! Only intervals of hours, minutes, seconds, milliseconds and microseconds
//! are read, since adding days, months or years to a timestamp depends on the
//! calendar and the session time zone. Of PostgreSQL's spellings, the reader
//! accepts the ones with a single reading:
//!
//! * quantities such as `1 hour -30 minutes`, each an integer with an optional
//!   sign followed by a unit, each unit at most once, refusing a leading `-`
//!   when no later quantity has a sign, since the `sql_standard` interval style
//!   applies that `-` to every quantity;
//! * one clock field such as `-01:30`, `00:05:00` or `00:00:01.5`, as
//!   PostgreSQL prints intervals, with two-digit minutes and seconds below 60
//!   and at most six fractional digits;
//!
//! either one optionally preceded by `@` and followed by `ago`, which negates
//! the interval.

/// Microseconds in a second.
const SECOND: i128 = 1_000_000;

/// Microseconds in a minute.
const MINUTE: i128 = 60 * SECOND;

/// Microseconds in an hour.
const HOUR: i128 = 60 * MINUTE;

/// Microseconds of the longest interval read, five thousand 365-day years, so
/// that shifting a current time before the year 5000 by it stays within the
/// years -9999 to 9999 that `jiff::Timestamp` represents.
const LONGEST: i128 = 5_000 * 365 * 24 * HOUR;

/// Returns the microseconds of the interval text, or `None` when the interval
/// has no fixed length or its spelling is not one the reader accepts.
pub(super) fn fixed_microseconds(text: &str) -> Option<i64> {
    let mut fields: Vec<&str> = text.split_ascii_whitespace().collect();
    if let Some(first) = fields.first_mut() {
        if *first == "@" {
            fields.remove(0);
        } else if let Some(rest) = first.strip_prefix('@') {
            *first = rest;
        }
    }
    let ago = fields.last().is_some_and(|last| last.eq_ignore_ascii_case("ago"));
    if ago {
        fields.pop();
    }
    let microseconds = match fields.as_slice() {
        [] => return None,
        [clock] if clock.contains(':') => clock_microseconds(clock)?,
        quantities => quantities_microseconds(quantities)?,
    };
    if microseconds.abs() > LONGEST {
        return None;
    }
    i64::try_from(if ago { -microseconds } else { microseconds }).ok()
}

/// Splits a leading `+` or `-` off the field, returning whether it was `-`.
fn split_sign(field: &str) -> (bool, &str) {
    match field.as_bytes().first() {
        Some(b'-') => (true, &field[1..]),
        Some(b'+') => (false, &field[1..]),
        _ => (false, field),
    }
}

/// Returns the value of a non-empty run of ASCII digits.
fn digits(text: &str) -> Option<i128> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Returns the value of two ASCII digits below 60.
fn sexagesimal(text: &str) -> Option<i128> {
    if text.len() != 2 {
        return None;
    }
    digits(text).filter(|value| *value < 60)
}

/// Returns the microseconds of a clock field, `[sign]H:MM` or
/// `[sign]H:MM:SS[.ffffff]`.
fn clock_microseconds(field: &str) -> Option<i128> {
    let (negative, clock) = split_sign(field);
    let mut parts = clock.split(':');
    let hours = digits(parts.next()?)?;
    let minutes = sexagesimal(parts.next()?)?;
    let seconds = match parts.next() {
        None => 0,
        Some(seconds) => {
            match seconds.split_once('.') {
                None => sexagesimal(seconds)? * SECOND,
                Some((whole, fraction)) if fraction.len() <= 6 => {
                    let scale = 10_i128.pow(6 - u32::try_from(fraction.len()).ok()?);
                    sexagesimal(whole)? * SECOND + digits(fraction)? * scale
                }
                Some(_) => return None,
            }
        }
    };
    if parts.next().is_some() {
        return None;
    }
    let microseconds = hours.checked_mul(HOUR)?.checked_add(minutes * MINUTE + seconds)?;
    Some(if negative { -microseconds } else { microseconds })
}

/// Returns the microseconds in one of the unit.
fn unit_microseconds(unit: &str) -> Option<i128> {
    Some(match unit.to_ascii_lowercase().as_str() {
        "us" | "usec" | "usecs" | "useconds" | "microsecond" | "microseconds" => 1,
        "ms" | "msec" | "msecs" | "mseconds" | "millisecond" | "milliseconds" => 1_000,
        "s" | "sec" | "secs" | "second" | "seconds" => SECOND,
        "m" | "min" | "mins" | "minute" | "minutes" => MINUTE,
        "h" | "hr" | "hrs" | "hour" | "hours" => HOUR,
        _ => return None,
    })
}

/// Returns the microseconds of a sequence of quantities, each an integer
/// with an optional sign followed, with or without whitespace, by a unit.
fn quantities_microseconds(fields: &[&str]) -> Option<i128> {
    let leading_minus = fields.first().is_some_and(|first| first.starts_with('-'));
    let mut fields = fields.iter();
    let mut seen_units = Vec::new();
    let mut later_sign = false;
    let mut total: i128 = 0;
    while let Some(field) = fields.next() {
        later_sign |= !seen_units.is_empty() && field.starts_with(['+', '-']);
        let (negative, quantity) = split_sign(field);
        let unit_start =
            quantity.find(|character: char| !character.is_ascii_digit()).unwrap_or(quantity.len());
        let (number, glued_unit) = quantity.split_at(unit_start);
        let unit = if glued_unit.is_empty() { *fields.next()? } else { glued_unit };
        let unit = unit_microseconds(unit)?;
        if seen_units.contains(&unit) {
            return None;
        }
        seen_units.push(unit);
        let microseconds = digits(number)?.checked_mul(unit)?;
        total = if negative {
            total.checked_sub(microseconds)?
        } else {
            total.checked_add(microseconds)?
        };
    }
    if leading_minus && seen_units.len() > 1 && !later_sign {
        return None;
    }
    Some(total)
}
