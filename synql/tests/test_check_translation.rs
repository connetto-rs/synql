//! Generated check constraints accept a row when the SQL expression is TRUE
//! or NULL and reject it when it is FALSE, as PostgreSQL does.
//!
//! The test generates a workspace, adds a crate of validation functions and a
//! crate of assertions over the generated validation code, and runs the
//! assertions with `cargo test`.

use std::process::Command;

use sql_traits::prelude::ParserDB;
use sqlparser::dialect::PostgreSqlDialect;
use synql::prelude::*;

const SCHEMA: &str = "
CREATE FUNCTION is_lowercase(TEXT) RETURNS BOOLEAN AS 'SELECT $1 = lower($1)' LANGUAGE SQL;
CREATE FUNCTION is_shorter(TEXT, TEXT) RETURNS BOOLEAN AS 'SELECT length($1) < length($2)' LANGUAGE SQL;
CREATE FUNCTION is_positive(INT) RETURNS BOOLEAN AS 'SELECT $1 > 0' LANGUAGE SQL;
CREATE TABLE ranges (
    id INT PRIMARY KEY,
    low INT NOT NULL,
    high INT,
    CHECK (low + 1 < high),
    CHECK (high > 0 AND low >= 0)
);
CREATE TABLE labels (
    id INT PRIMARY KEY,
    label TEXT
        CHECK (label IS NULL OR length(label) > 2)
        CHECK (label <> 'none' OR label IS NULL)
        CHECK (label IS NOT NULL OR label = 'x')
);
CREATE TABLE scores (
    id INT PRIMARY KEY,
    score INT NOT NULL CHECK ((score * 2)::INT >= 0),
    bonus INT NOT NULL CHECK (100 > bonus)
);
CREATE TABLE overrides (
    id INT PRIMARY KEY,
    low INT NOT NULL,
    high INT,
    note TEXT,
    CHECK (high IS NULL OR high > low OR note = 'override')
);
CREATE TABLE windows (
    id INT PRIMARY KEY,
    opens INT,
    closes INT,
    CHECK (opens IS NULL OR (closes IS NOT NULL AND opens < closes))
);
CREATE TABLE codes (
    id INT PRIMARY KEY,
    code TEXT CHECK (is_lowercase(code)),
    alias TEXT,
    CHECK (is_lowercase(alias) OR code IS NOT NULL),
    CHECK (is_shorter(code, alias))
);
CREATE TABLE budgets (
    id INT PRIMARY KEY,
    spent INT NOT NULL,
    cap INT NOT NULL,
    CHECK ((10) + spent <= cap),
    CHECK ((cap - spent) * 2 / 2 % 1000 >= 0),
    CHECK (cap > spent),
    CHECK (cap >= spent)
);
CREATE TABLE flags (
    id INT PRIMARY KEY,
    enabled BOOLEAN NOT NULL,
    label TEXT,
    CHECK (enabled = true OR label IS NOT NULL)
);
CREATE TABLE switches (
    id INT PRIMARY KEY,
    on_off BOOLEAN NOT NULL
        CHECK (false OR on_off)
        CHECK (on_off OR (on_off = false AND on_off = false))
);
CREATE TABLE memos (
    id INT PRIMARY KEY,
    memo TEXT,
    priority INT NOT NULL CHECK (is_positive(priority)),
    CHECK (length(memo) > 2 OR priority > 0),
    CHECK (length(memo) <= 50 AND priority < 100)
);
CREATE TABLE deadlines (
    id INT PRIMARY KEY,
    due TIMESTAMPTZ,
    opened TIMESTAMPTZ NOT NULL CHECK (opened <= NOW()) CHECK (opened < NOW()),
    closed TIMESTAMPTZ NOT NULL CHECK (closed > NOW()),
    CHECK (NOW() < due OR opened < due),
    CHECK (due < NOW() AND opened < due)
);
CREATE TABLE docs (
    id INT PRIMARY KEY,
    body JSONB,
    meta JSONB NOT NULL,
    CHECK (body IS NOT NULL OR meta = meta)
);
CREATE SCHEMA rules;
CREATE FUNCTION rules.is_even(INT) RETURNS BOOLEAN AS 'SELECT $1 % 2 = 0' LANGUAGE SQL;
CREATE TABLE tallies (
    id INT PRIMARY KEY,
    tally INT NOT NULL CHECK (rules.is_even(tally))
);
CREATE TABLE contacts (
    id INT PRIMARY KEY,
    email TEXT NOT NULL CHECK (email LIKE '%_@_%' AND email = lower(email)),
    handle TEXT,
    nickname TEXT,
    trusted BOOLEAN NOT NULL,
    CHECK (handle NOT LIKE 'admin%' OR trusted),
    CHECK (nickname = lower(nickname) OR trusted),
    CHECK (upper(nickname) ILIKE 'X%' OR nickname IS NULL OR trusted)
);
CREATE TABLE patterns (
    id INT PRIMARY KEY,
    path TEXT NOT NULL CHECK (path LIKE '/%/%.rs'),
    code TEXT NOT NULL CHECK (code LIKE 'a\\_b%c'),
    tag TEXT NOT NULL CHECK (tag ILIKE 'x%Y_z'),
    shout TEXT NOT NULL CHECK (shout = upper(shout)),
    anything TEXT NOT NULL CHECK (anything LIKE '%%'),
    exact TEXT NOT NULL CHECK (exact LIKE 'a_c'),
    spread TEXT NOT NULL CHECK (spread LIKE '%a%b%')
);
CREATE TABLE digests (
    id INT PRIMARY KEY,
    token_hash BYTEA NOT NULL CHECK (octet_length(token_hash) = 32),
    digest BYTEA,
    note TEXT,
    trusted BOOLEAN NOT NULL,
    CHECK (octet_length(digest) = 4 OR trusted),
    CHECK (octet_length(note) <= 4 OR trusted)
);
CREATE TABLE tiers (
    id INT PRIMARY KEY,
    tier INT CHECK (tier IN (1, 2)),
    kind TEXT NOT NULL CHECK (kind NOT IN ('x', 'y')),
    cap INT,
    CHECK (tier IN (1, cap)),
    CHECK (upper(kind) IN ('Z', 'W', kind))
);
CREATE TABLE stamps (
    id INT PRIMARY KEY,
    verbose TIMESTAMPTZ NOT NULL CHECK (verbose <= now() + INTERVAL '5 minutes'),
    glued TIMESTAMPTZ NOT NULL CHECK (glued <= now() + interval '5min'),
    shouted TIMESTAMPTZ NOT NULL CHECK (shouted <= NOW() + INTERVAL '+5 MINUTES'),
    seconds TIMESTAMPTZ NOT NULL CHECK (seconds <= now() + INTERVAL '@ 300 secs'),
    clock TIMESTAMPTZ NOT NULL CHECK (clock <= now() + '00:05:00'::interval),
    short_clock TIMESTAMPTZ NOT NULL CHECK (short_clock <= (now() + '0:05'::INTERVAL)),
    mixed TIMESTAMPTZ NOT NULL CHECK (mixed <= now() + INTERVAL '1 hour -55 m'),
    ago TIMESTAMPTZ NOT NULL CHECK (ago <= now() + INTERVAL '-5 minutes ago'),
    millis TIMESTAMPTZ NOT NULL CHECK (millis <= now() + INTERVAL '300000 ms'),
    micros TIMESTAMPTZ NOT NULL CHECK (micros <= now() + INTERVAL '300000000 usec'),
    fraction TIMESTAMPTZ NOT NULL CHECK (fraction <= now() + '00:04:59.999999'::interval)
);
CREATE TABLE leases (
    id INT PRIMARY KEY,
    ended TIMESTAMPTZ CHECK (ended > now() - '01:30:00'::interval),
    renewed TIMESTAMPTZ CHECK (INTERVAL '@ 2 hours 30 secs ago' + now() < renewed),
    reminded TIMESTAMPTZ CHECK (now() + INTERVAL '-1 hour +90 minutes 250 milliseconds' >= reminded),
    rewound TIMESTAMPTZ CHECK (rewound >= now() - INTERVAL '-00:10:00'),
    started TIMESTAMPTZ NOT NULL,
    CHECK (started <= now() + INTERVAL '5 minutes' OR ended IS NOT NULL)
);
CREATE TABLE blinks (
    id INT PRIMARY KEY,
    clock TIMESTAMPTZ NOT NULL CHECK (clock <= now() + '00:00:01.75'::interval),
    millis TIMESTAMPTZ NOT NULL CHECK (millis <= now() + INTERVAL '1750 ms'),
    micros TIMESTAMPTZ NOT NULL CHECK (micros <= now() + INTERVAL '1 s 750000 us')
);
CREATE TABLE readings (
    id INT PRIMARY KEY,
    volume DOUBLE PRECISION CHECK (volume >= 0.0 AND volume < 'Infinity'::double precision),
    depth DOUBLE PRECISION NOT NULL CHECK ('-inf'::FLOAT8 < depth) CHECK (depth <= ' 1.5e3 '::float8),
    ratio REAL NOT NULL CHECK (ratio < '+INFINITY'::real)
);
CREATE TABLE namespaces (
    id INT PRIMARY KEY,
    owner INT,
    reserved BOOLEAN NOT NULL,
    retired BOOLEAN,
    hidden BOOLEAN,
    archived BOOLEAN NOT NULL CHECK (NOT archived),
    slug TEXT NOT NULL CHECK (NOT (slug LIKE 'x%')) CHECK (NOT (slug ILIKE '%z')),
    CHECK (NOT reserved OR owner IS NULL),
    CHECK (NOT retired OR owner IS NOT NULL),
    CHECK (NOT (retired AND hidden))
);
CREATE TABLE principals (
    id INT PRIMARY KEY,
    kind TEXT NOT NULL,
    namespace_id UUID NOT NULL,
    parent_id UUID CHECK (parent_id <> '{A0EEBC99-9C0B4EF8-BB6D6BB9-BD380A11}'::uuid),
    rank INT NOT NULL CHECK (rank < '100'),
    CHECK (kind NOT IN ('user', 'everyone', 'signed_in') OR namespace_id = 'bc3cf2e2-768c-4af9-bf86-65f23378a212')
);
CREATE TABLE teams (
    namespace_id INT NOT NULL,
    id INT NOT NULL,
    parent_namespace_id INT,
    parent_id INT,
    PRIMARY KEY (namespace_id, id),
    CHECK ((parent_namespace_id, parent_id) <> (namespace_id, id))
);
CREATE TABLE mirrors (
    id INT PRIMARY KEY,
    low INT NOT NULL,
    high INT NOT NULL,
    copy_low INT NOT NULL,
    copy_high INT,
    label TEXT NOT NULL,
    CHECK ((low, high) <> (high, low)),
    CHECK ((low, high) = (copy_low, copy_high)),
    CHECK (((copy_low, low)) = (low, copy_low)),
    CHECK ((label, low) <> ('none', 0))
);
";

const CHECKS_MANIFEST: &str = r#"[package]
name = "checks"
version = "0.1.0"
edition = "2024"
"#;

const CHECKS_LIB: &str = r#"//! Validation functions the schema's check constraints call.

/// A text that breaks a validation function.
#[derive(Debug)]
pub struct Rejected(&'static str);

impl core::fmt::Display for Rejected {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.0)
    }
}

impl core::error::Error for Rejected {}

/// Accepts a text without uppercase letters.
///
/// # Errors
///
/// Rejects a text with an uppercase letter.
pub fn is_lowercase(value: &str) -> Result<(), Rejected> {
    if value.chars().any(char::is_uppercase) { Err(Rejected("not lowercase")) } else { Ok(()) }
}

/// Accepts a first text shorter than the second.
///
/// # Errors
///
/// Rejects a first text at least as long as the second.
pub fn is_shorter(first: &str, second: &str) -> Result<(), Rejected> {
    if first.chars().count() < second.chars().count() { Ok(()) } else { Err(Rejected("not shorter")) }
}

/// Accepts a positive number.
///
/// # Errors
///
/// Rejects zero and negative numbers.
pub fn is_positive(value: &i32) -> Result<(), Rejected> {
    if *value > 0 { Ok(()) } else { Err(Rejected("not positive")) }
}

/// Accepts an even number.
///
/// # Errors
///
/// Rejects an odd number.
pub fn is_even(value: &i32) -> Result<(), Rejected> {
    if *value % 2 == 0 { Ok(()) } else { Err(Rejected("not even")) }
}
"#;

const ASSERTIONS_MANIFEST: &str = r#"[package]
name = "assertions"
version = "0.1.0"
edition = "2024"

[dependencies]
diesel-builders.workspace = true
validation-errors.workspace = true
synql-check-translation-ranges.workspace = true
synql-check-translation-labels.workspace = true
synql-check-translation-scores.workspace = true
synql-check-translation-overrides.workspace = true
synql-check-translation-windows.workspace = true
synql-check-translation-codes.workspace = true
synql-check-translation-budgets.workspace = true
synql-check-translation-flags.workspace = true
synql-check-translation-switches.workspace = true
synql-check-translation-memos.workspace = true
synql-check-translation-deadlines.workspace = true
synql-check-translation-docs.workspace = true
synql-check-translation-tallies.workspace = true
synql-check-translation-contacts.workspace = true
synql-check-translation-patterns.workspace = true
synql-check-translation-digests.workspace = true
synql-check-translation-tiers.workspace = true
synql-check-translation-stamps.workspace = true
synql-check-translation-leases.workspace = true
synql-check-translation-blinks.workspace = true
synql-check-translation-readings.workspace = true
synql-check-translation-namespaces.workspace = true
synql-check-translation-principals.workspace = true
synql-check-translation-teams.workspace = true
synql-check-translation-mirrors.workspace = true
rosetta-utc.workspace = true
rosetta-uuid.workspace = true
"#;

const ASSERTIONS_TEST: &str = r#"//! Assertions over the generated validation code.

use diesel_builders::{SetColumn, TableExt, ValidateColumn, ValidateRecord};
use validation_errors::{ValidationError, ValidationErrorKind};

fn is_check_violation(result: Result<(), ValidationError>) -> bool {
    matches!(result, Err(error) if matches!(error.kind(), ValidationErrorKind::CheckViolated { .. }))
}

fn is_function_rejection(result: Result<(), ValidationError>) -> bool {
    matches!(result, Err(error) if matches!(error.kind(), ValidationErrorKind::Generic { .. }))
}

#[test]
fn arithmetic_comparisons_pass_on_null_and_fail_on_false() {
    use synql_check_translation_ranges::ranges;
    let validate = |low: i32, high: Option<i32>| {
        let mut values = ranges::table::empty_new_values();
        SetColumn::<ranges::low>::set_column(&mut values, low);
        SetColumn::<ranges::high>::set_column(&mut values, high);
        ValidateRecord::<ranges::table>::validate_record(&values)
    };
    assert!(validate(1, Some(5)).is_ok());
    assert!(validate(1, None).is_ok());
    assert!(is_check_violation(validate(4, Some(5))));
}

#[test]
fn single_column_checks_validate_the_value() {
    use synql_check_translation_labels::labels;
    use synql_check_translation_scores::scores;
    type Labels = <labels::table as TableExt>::NewValues;
    type Scores = <scores::table as TableExt>::NewValues;

    assert!(<Labels as ValidateColumn<labels::label>>::validate_column(&"abc".to_owned()).is_ok());
    assert!(is_check_violation(<Labels as ValidateColumn<labels::label>>::validate_column(
        &"ab".to_owned()
    )));
    assert!(is_check_violation(<Labels as ValidateColumn<labels::label>>::validate_column(
        &"none".to_owned()
    )));
    assert!(<Scores as ValidateColumn<scores::score>>::validate_column(&3).is_ok());
    assert!(is_check_violation(<Scores as ValidateColumn<scores::score>>::validate_column(&-1)));
    assert!(<Scores as ValidateColumn<scores::bonus>>::validate_column(&99).is_ok());
    assert!(matches!(
        <Scores as ValidateColumn<scores::bonus>>::validate_column(&100),
        Err(error) if matches!(error.kind(), ValidationErrorKind::MustBeStrictlySmallerThanScalar(..))
    ));
}

#[test]
fn or_follows_three_valued_logic() {
    use synql_check_translation_overrides::overrides;
    let validate = |low: i32, high: Option<i32>, note: Option<&str>| {
        let mut values = overrides::table::empty_new_values();
        SetColumn::<overrides::low>::set_column(&mut values, low);
        SetColumn::<overrides::high>::set_column(&mut values, high);
        SetColumn::<overrides::note>::set_column(&mut values, note.map(str::to_owned));
        ValidateRecord::<overrides::table>::validate_record(&values)
    };
    assert!(validate(5, None, None).is_ok());
    assert!(validate(5, Some(9), None).is_ok());
    assert!(validate(5, Some(1), None).is_ok());
    assert!(validate(5, Some(1), Some("override")).is_ok());
    assert!(is_check_violation(validate(5, Some(1), Some("locked"))));
}

#[test]
fn in_lists_match_any_item_and_not_in_lists_none() {
    use synql_check_translation_tiers::tiers;
    type Tiers = <tiers::table as TableExt>::NewValues;
    assert!(<Tiers as ValidateColumn<tiers::tier>>::validate_column(&2).is_ok());
    assert!(is_check_violation(<Tiers as ValidateColumn<tiers::tier>>::validate_column(&3)));
    assert!(<Tiers as ValidateColumn<tiers::kind>>::validate_column(&"z".to_owned()).is_ok());
    assert!(is_check_violation(<Tiers as ValidateColumn<tiers::kind>>::validate_column(
        &"y".to_owned()
    )));
    assert!(<Tiers as ValidateColumn<tiers::kind>>::validate_column(&"w".to_owned()).is_ok());
    assert!(<Tiers as ValidateColumn<tiers::kind>>::validate_column(&"Q".to_owned()).is_ok());
    assert!(is_check_violation(<Tiers as ValidateColumn<tiers::kind>>::validate_column(
        &"q".to_owned()
    )));

    let validate = |tier: Option<i32>, cap: Option<i32>| {
        let mut values = tiers::table::empty_new_values();
        SetColumn::<tiers::tier>::set_column(&mut values, tier);
        SetColumn::<tiers::kind>::set_column(&mut values, "z".to_owned());
        SetColumn::<tiers::cap>::set_column(&mut values, cap);
        ValidateRecord::<tiers::table>::validate_record(&values)
    };
    assert!(validate(None, Some(2)).is_ok());
    assert!(validate(Some(1), Some(2)).is_ok());
    assert!(validate(Some(2), Some(2)).is_ok());
    assert!(validate(Some(2), None).is_ok());
    assert!(is_check_violation(validate(Some(2), Some(3))));
}

#[test]
fn and_follows_three_valued_logic() {
    use synql_check_translation_windows::windows;
    let validate = |opens: Option<i32>, closes: Option<i32>| {
        let mut values = windows::table::empty_new_values();
        SetColumn::<windows::opens>::set_column(&mut values, opens);
        SetColumn::<windows::closes>::set_column(&mut values, closes);
        ValidateRecord::<windows::table>::validate_record(&values)
    };
    assert!(validate(None, None).is_ok());
    assert!(validate(Some(1), Some(2)).is_ok());
    assert!(is_check_violation(validate(Some(1), None)));
    assert!(is_check_violation(validate(Some(3), Some(2))));
}

#[test]
fn validation_functions_reject_through_their_own_error_or_the_check() {
    use synql_check_translation_codes::codes;
    type Codes = <codes::table as TableExt>::NewValues;
    let validate = |code: Option<&str>, alias: Option<&str>| {
        let mut values = codes::table::empty_new_values();
        SetColumn::<codes::code>::set_column(&mut values, code.map(str::to_owned));
        SetColumn::<codes::alias>::set_column(&mut values, alias.map(str::to_owned));
        ValidateRecord::<codes::table>::validate_record(&values)
    };

    assert!(<Codes as ValidateColumn<codes::code>>::validate_column(&"abc".to_owned()).is_ok());
    assert!(is_function_rejection(<Codes as ValidateColumn<codes::code>>::validate_column(
        &"Abc".to_owned()
    )));
    assert!(validate(Some("ab"), Some("abc")).is_ok());
    assert!(validate(None, Some("abc")).is_ok());
    assert!(validate(None, None).is_ok());
    assert!(is_check_violation(validate(None, Some("Abc"))));
    assert!(is_function_rejection(validate(Some("abcd"), Some("abc"))));
}

#[test]
fn arithmetic_with_leading_literals_and_every_operator() {
    use synql_check_translation_budgets::budgets;
    let validate = |spent: i32, cap: i32| {
        let mut values = budgets::table::empty_new_values();
        SetColumn::<budgets::spent>::set_column(&mut values, spent);
        SetColumn::<budgets::cap>::set_column(&mut values, cap);
        ValidateRecord::<budgets::table>::validate_record(&values)
    };
    assert!(validate(5, 20).is_ok());
    assert!(is_check_violation(validate(15, 20)));
}

#[test]
fn boolean_columns_and_literals() {
    use synql_check_translation_flags::flags;
    use synql_check_translation_switches::switches;
    type Switches = <switches::table as TableExt>::NewValues;
    let validate = |enabled: bool, label: Option<&str>| {
        let mut values = flags::table::empty_new_values();
        SetColumn::<flags::enabled>::set_column(&mut values, enabled);
        SetColumn::<flags::label>::set_column(&mut values, label.map(str::to_owned));
        ValidateRecord::<flags::table>::validate_record(&values)
    };
    assert!(validate(true, None).is_ok());
    assert!(validate(false, Some("x")).is_ok());
    assert!(is_check_violation(validate(false, None)));
    assert!(<Switches as ValidateColumn<switches::on_off>>::validate_column(&true).is_ok());
    assert!(is_check_violation(<Switches as ValidateColumn<switches::on_off>>::validate_column(
        &false
    )));
}

#[test]
fn not_negates_booleans_and_leaves_null_null() {
    use synql_check_translation_namespaces::namespaces;
    type Namespaces = <namespaces::table as TableExt>::NewValues;
    let validate =
        |reserved: bool, retired: Option<bool>, hidden: Option<bool>, owner: Option<i32>| {
            let mut values = namespaces::table::empty_new_values();
            SetColumn::<namespaces::owner>::set_column(&mut values, owner);
            SetColumn::<namespaces::reserved>::set_column(&mut values, reserved);
            SetColumn::<namespaces::retired>::set_column(&mut values, retired);
            SetColumn::<namespaces::hidden>::set_column(&mut values, hidden);
            SetColumn::<namespaces::archived>::set_column(&mut values, false);
            SetColumn::<namespaces::slug>::set_column(&mut values, "a".to_owned());
            ValidateRecord::<namespaces::table>::validate_record(&values)
        };
    assert!(validate(false, None, None, Some(1)).is_ok());
    assert!(validate(true, None, None, None).is_ok());
    assert!(is_check_violation(validate(true, None, None, Some(1))));
    assert!(validate(false, Some(false), None, None).is_ok());
    assert!(is_check_violation(validate(false, Some(true), None, None)));
    assert!(validate(false, Some(true), None, Some(1)).is_ok());
    assert!(is_check_violation(validate(false, Some(true), Some(true), Some(1))));
    assert!(validate(false, Some(true), Some(false), Some(1)).is_ok());
    assert!(<Namespaces as ValidateColumn<namespaces::archived>>::validate_column(&false).is_ok());
    assert!(is_check_violation(
        <Namespaces as ValidateColumn<namespaces::archived>>::validate_column(&true)
    ));
    assert!(<Namespaces as ValidateColumn<namespaces::slug>>::validate_column(&"ax".to_owned()).is_ok());
    assert!(is_check_violation(<Namespaces as ValidateColumn<namespaces::slug>>::validate_column(
        &"xa".to_owned()
    )));
    assert!(is_check_violation(<Namespaces as ValidateColumn<namespaces::slug>>::validate_column(
        &"aZ".to_owned()
    )));
}

#[test]
fn lengths_of_null_text_and_functions_of_numbers() {
    use synql_check_translation_memos::memos;
    type Memos = <memos::table as TableExt>::NewValues;
    let validate = |memo: Option<&str>, priority: i32| {
        let mut values = memos::table::empty_new_values();
        SetColumn::<memos::memo>::set_column(&mut values, memo.map(str::to_owned));
        SetColumn::<memos::priority>::set_column(&mut values, priority);
        ValidateRecord::<memos::table>::validate_record(&values)
    };
    assert!(validate(None, 0).is_ok());
    assert!(validate(Some("abc"), 0).is_ok());
    assert!(is_check_violation(validate(Some("ab"), 0)));
    assert!(<Memos as ValidateColumn<memos::priority>>::validate_column(&1).is_ok());
    assert!(is_function_rejection(<Memos as ValidateColumn<memos::priority>>::validate_column(&0)));
}

#[test]
fn schema_qualified_calls_reach_their_function() {
    use synql_check_translation_tallies::tallies;
    type Tallies = <tallies::table as TableExt>::NewValues;
    assert!(<Tallies as ValidateColumn<tallies::tally>>::validate_column(&2).is_ok());
    assert!(is_function_rejection(<Tallies as ValidateColumn<tallies::tally>>::validate_column(&3)));
}

#[test]
fn like_matches_postgresql_wildcards_and_casing() {
    use synql_check_translation_contacts::contacts;
    use synql_check_translation_patterns::patterns;
    type Contacts = <contacts::table as TableExt>::NewValues;
    type Patterns = <patterns::table as TableExt>::NewValues;
    let email =
        |value: &str| <Contacts as ValidateColumn<contacts::email>>::validate_column(&value.to_owned());
    let path =
        |value: &str| <Patterns as ValidateColumn<patterns::path>>::validate_column(&value.to_owned());
    let code =
        |value: &str| <Patterns as ValidateColumn<patterns::code>>::validate_column(&value.to_owned());
    let tag =
        |value: &str| <Patterns as ValidateColumn<patterns::tag>>::validate_column(&value.to_owned());
    let shout =
        |value: &str| <Patterns as ValidateColumn<patterns::shout>>::validate_column(&value.to_owned());
    let anything = |value: &str| {
        <Patterns as ValidateColumn<patterns::anything>>::validate_column(&value.to_owned())
    };
    let exact =
        |value: &str| <Patterns as ValidateColumn<patterns::exact>>::validate_column(&value.to_owned());
    let spread = |value: &str| {
        <Patterns as ValidateColumn<patterns::spread>>::validate_column(&value.to_owned())
    };

    assert!(email("a@b").is_ok());
    assert!(email("first.last@example.org").is_ok());
    assert!(is_check_violation(email("@b")));
    assert!(is_check_violation(email("a@")));
    assert!(is_check_violation(email("ab")));
    assert!(is_check_violation(email("A@b")));

    assert!(path("/src/main.rs").is_ok());
    assert!(path("/a/.rs").is_ok());
    assert!(path("/a/b/c.rs").is_ok());
    assert!(is_check_violation(path("/main.rs")));
    assert!(is_check_violation(path("/a/b.rsx")));
    assert!(is_check_violation(path("src/a.rs")));

    assert!(code("a_bc").is_ok());
    assert!(code("a_bxyzc").is_ok());
    assert!(is_check_violation(code("axbc")));
    assert!(is_check_violation(code("a_bcd")));
    assert!(is_check_violation(code("a\\_bc")));

    assert!(tag("XaaYbZ").is_ok());
    assert!(tag("xAyBz").is_ok());
    assert!(tag("xybz").is_ok());
    assert!(is_check_violation(tag("xyz")));
    assert!(is_check_violation(tag("ayBz")));

    assert!(shout("ÀBC").is_ok());
    assert!(is_check_violation(shout("AbC")));

    assert!(anything("").is_ok());

    assert!(exact("abc").is_ok());
    assert!(exact("aÀc").is_ok());
    assert!(is_check_violation(exact("ac")));
    assert!(is_check_violation(exact("abcd")));
    assert!(is_check_violation(exact("xbc")));

    assert!(spread("ab").is_ok());
    assert!(spread("xaxxbx").is_ok());
    assert!(spread("bab").is_ok());
    assert!(is_check_violation(spread("ba")));
    assert!(is_check_violation(spread("")));
}

#[test]
fn text_functions_follow_three_valued_logic() {
    use synql_check_translation_contacts::contacts;
    let validate = |handle: Option<&str>, nickname: Option<&str>, trusted: bool| {
        let mut values = contacts::table::empty_new_values();
        SetColumn::<contacts::handle>::set_column(&mut values, handle.map(str::to_owned));
        SetColumn::<contacts::nickname>::set_column(&mut values, nickname.map(str::to_owned));
        SetColumn::<contacts::trusted>::set_column(&mut values, trusted);
        ValidateRecord::<contacts::table>::validate_record(&values)
    };
    assert!(validate(None, None, false).is_ok());
    assert!(validate(Some("root"), None, false).is_ok());
    assert!(validate(Some("admin1"), None, true).is_ok());
    assert!(is_check_violation(validate(Some("admin1"), None, false)));
    assert!(validate(None, Some("xavier"), false).is_ok());
    assert!(is_check_violation(validate(None, Some("Xavier"), false)));
    assert!(is_check_violation(validate(None, Some("yves"), false)));
    assert!(validate(None, Some("Yves"), true).is_ok());
}

#[test]
fn octet_length_counts_bytes() {
    use synql_check_translation_digests::digests;
    type Digests = <digests::table as TableExt>::NewValues;
    let validate = |digest: Option<Vec<u8>>, note: Option<&str>, trusted: bool| {
        let mut values = digests::table::empty_new_values();
        SetColumn::<digests::digest>::set_column(&mut values, digest);
        SetColumn::<digests::note>::set_column(&mut values, note.map(str::to_owned));
        SetColumn::<digests::trusted>::set_column(&mut values, trusted);
        ValidateRecord::<digests::table>::validate_record(&values)
    };
    assert!(<Digests as ValidateColumn<digests::token_hash>>::validate_column(&vec![0; 32]).is_ok());
    assert!(is_check_violation(
        <Digests as ValidateColumn<digests::token_hash>>::validate_column(&vec![0; 31])
    ));
    assert!(validate(None, None, false).is_ok());
    assert!(validate(Some(vec![1, 2, 3, 4]), None, false).is_ok());
    assert!(is_check_violation(validate(Some(vec![1]), None, false)));
    assert!(validate(None, Some("éé"), false).is_ok());
    assert!(is_check_violation(validate(None, Some("ééé"), false)));
    assert!(validate(None, Some("abcde"), true).is_ok());
}

/// Returns the current time shifted by the milliseconds.
fn shifted_millis(milliseconds: i64) -> rosetta_utc::TimestampUTC {
    let now = *rosetta_utc::TimestampUTC::now();
    let offset = core::time::Duration::from_millis(milliseconds.unsigned_abs());
    rosetta_utc::TimestampUTC::from(if milliseconds < 0 { now - offset } else { now + offset })
}

/// Returns the current time shifted by the seconds.
fn shifted(seconds: i64) -> rosetta_utc::TimestampUTC {
    shifted_millis(seconds * 1_000)
}

#[test]
fn now_shifts_by_every_spelling_of_five_minutes() {
    use synql_check_translation_stamps::stamps;
    type Stamps = <stamps::table as TableExt>::NewValues;
    macro_rules! assert_five_minutes {
        ($($column:ident),*) => {$(
            assert!(<Stamps as ValidateColumn<stamps::$column>>::validate_column(&shifted(-86_400)).is_ok());
            assert!(<Stamps as ValidateColumn<stamps::$column>>::validate_column(&shifted(240)).is_ok());
            assert!(
                is_check_violation(<Stamps as ValidateColumn<stamps::$column>>::validate_column(&shifted(360))),
                stringify!($column)
            );
        )*};
    }
    assert_five_minutes!(
        verbose, glued, shouted, seconds, clock, short_clock, mixed, ago, millis, micros, fraction
    );
}

#[test]
fn now_shifts_keep_fractions_of_a_second() {
    use synql_check_translation_blinks::blinks;
    type Blinks = <blinks::table as TableExt>::NewValues;
    macro_rules! assert_one_and_three_quarter_seconds {
        ($($column:ident),*) => {$(
            assert!(
                <Blinks as ValidateColumn<blinks::$column>>::validate_column(&shifted_millis(1_500)).is_ok(),
                stringify!($column)
            );
            assert!(
                is_check_violation(<Blinks as ValidateColumn<blinks::$column>>::validate_column(&shifted_millis(2_000))),
                stringify!($column)
            );
        )*};
    }
    assert_one_and_three_quarter_seconds!(clock, millis, micros);
}

#[test]
fn now_shifts_subtract_commute_and_follow_three_valued_logic() {
    use synql_check_translation_leases::leases;
    type Leases = <leases::table as TableExt>::NewValues;
    assert!(<Leases as ValidateColumn<leases::ended>>::validate_column(&shifted(-5_340)).is_ok());
    assert!(is_check_violation(<Leases as ValidateColumn<leases::ended>>::validate_column(&shifted(-5_460))));
    assert!(<Leases as ValidateColumn<leases::renewed>>::validate_column(&shifted(-7_200)).is_ok());
    assert!(is_check_violation(<Leases as ValidateColumn<leases::renewed>>::validate_column(&shifted(-7_290))));
    assert!(<Leases as ValidateColumn<leases::reminded>>::validate_column(&shifted(1_740)).is_ok());
    assert!(is_check_violation(<Leases as ValidateColumn<leases::reminded>>::validate_column(&shifted(1_860))));
    assert!(<Leases as ValidateColumn<leases::rewound>>::validate_column(&shifted(660)).is_ok());
    assert!(is_check_violation(<Leases as ValidateColumn<leases::rewound>>::validate_column(&shifted(540))));

    let validate = |started: i64, ended: Option<i64>| {
        let mut values = leases::table::empty_new_values();
        SetColumn::<leases::started>::set_column(&mut values, shifted(started));
        SetColumn::<leases::ended>::set_column(&mut values, ended.map(shifted));
        ValidateRecord::<leases::table>::validate_record(&values)
    };
    assert!(validate(240, None).is_ok());
    assert!(validate(360, Some(0)).is_ok());
    assert!(is_check_violation(validate(360, None)));
}

#[test]
fn float_casts_read_infinities_and_numeric_strings() {
    use synql_check_translation_readings::readings;
    type Readings = <readings::table as TableExt>::NewValues;
    let volume =
        |volume: f64| <Readings as ValidateColumn<readings::volume>>::validate_column(&volume);
    assert!(volume(0.0).is_ok());
    assert!(volume(f64::MAX).is_ok());
    assert!(volume(-1.0).is_err());
    assert!(is_check_violation(volume(f64::INFINITY)));
    assert!(is_check_violation(volume(f64::NAN)));

    let depth = |depth: f64| <Readings as ValidateColumn<readings::depth>>::validate_column(&depth);
    assert!(depth(f64::MIN).is_ok());
    assert!(depth(1_500.0).is_ok());
    assert!(is_check_violation(depth(f64::NEG_INFINITY)));
    assert!(is_check_violation(depth(1_500.5)));

    let ratio = |ratio: f32| <Readings as ValidateColumn<readings::ratio>>::validate_column(&ratio);
    assert!(ratio(f32::MAX).is_ok());
    assert!(is_check_violation(ratio(f32::INFINITY)));
    assert!(is_check_violation(ratio(f32::NAN)));
}

#[test]
fn quoted_literals_take_the_type_of_the_other_operand() {
    use synql_check_translation_principals::principals;
    type Principals = <principals::table as TableExt>::NewValues;
    let uuid = |text: &str| text.parse::<rosetta_uuid::Uuid>().expect("A valid UUID");
    let reserved = uuid("bc3cf2e2-768c-4af9-bf86-65f23378a212");
    let other = uuid("00000000-0000-0000-0000-000000000001");
    let validate = |kind: &str, namespace_id: rosetta_uuid::Uuid| {
        let mut values = principals::table::empty_new_values();
        SetColumn::<principals::kind>::set_column(&mut values, kind.to_owned());
        SetColumn::<principals::namespace_id>::set_column(&mut values, namespace_id);
        SetColumn::<principals::parent_id>::set_column(&mut values, None);
        SetColumn::<principals::rank>::set_column(&mut values, 1);
        ValidateRecord::<principals::table>::validate_record(&values)
    };
    assert!(validate("user", reserved).is_ok());
    assert!(validate("team", other).is_ok());
    assert!(is_check_violation(validate("everyone", other)));

    let parent_id =
        |id: rosetta_uuid::Uuid| <Principals as ValidateColumn<principals::parent_id>>::validate_column(&id);
    assert!(parent_id(other).is_ok());
    assert!(is_check_violation(parent_id(uuid("a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11"))));

    let rank = |rank: i32| <Principals as ValidateColumn<principals::rank>>::validate_column(&rank);
    assert!(rank(99).is_ok());
    assert!(matches!(
        rank(100),
        Err(error) if matches!(error.kind(), ValidationErrorKind::MustBeStrictlySmallerThanScalar(..))
    ));
}

#[test]
fn row_comparisons_follow_postgresql_row_semantics() {
    use synql_check_translation_mirrors::mirrors;
    use synql_check_translation_teams::teams;
    let team = |parent_namespace_id: Option<i32>, parent_id: Option<i32>| {
        let mut values = teams::table::empty_new_values();
        SetColumn::<teams::namespace_id>::set_column(&mut values, 1);
        SetColumn::<teams::id>::set_column(&mut values, 2);
        SetColumn::<teams::parent_namespace_id>::set_column(&mut values, parent_namespace_id);
        SetColumn::<teams::parent_id>::set_column(&mut values, parent_id);
        ValidateRecord::<teams::table>::validate_record(&values)
    };
    assert!(team(None, None).is_ok());
    assert!(team(Some(1), None).is_ok());
    assert!(team(Some(3), None).is_ok());
    assert!(team(Some(1), Some(3)).is_ok());
    assert!(team(Some(3), Some(2)).is_ok());
    assert!(matches!(
        team(Some(1), Some(2)),
        Err(error) if matches!(
            error.kind(),
            ValidationErrorKind::CheckViolated {
                fields: ["parent_namespace_id", "namespace_id", "parent_id", "id"],
                ..
            }
        )
    ));

    let mirror = |low: i32, high: i32, copy_low: i32, copy_high: Option<i32>, label: &str| {
        let mut values = mirrors::table::empty_new_values();
        SetColumn::<mirrors::low>::set_column(&mut values, low);
        SetColumn::<mirrors::high>::set_column(&mut values, high);
        SetColumn::<mirrors::copy_low>::set_column(&mut values, copy_low);
        SetColumn::<mirrors::copy_high>::set_column(&mut values, copy_high);
        SetColumn::<mirrors::label>::set_column(&mut values, label.to_owned());
        ValidateRecord::<mirrors::table>::validate_record(&values)
    };
    assert!(mirror(1, 2, 1, Some(2), "a").is_ok());
    assert!(mirror(1, 2, 1, None, "a").is_ok());
    assert!(is_check_violation(mirror(1, 2, 1, Some(3), "a")));
    assert!(is_check_violation(mirror(1, 2, 0, None, "a")));
    assert!(is_check_violation(mirror(2, 2, 2, Some(2), "a")));
    assert!(mirror(1, 2, 1, Some(2), "none").is_ok());
    assert!(mirror(0, 2, 0, Some(2), "nones").is_ok());
    assert!(is_check_violation(mirror(0, 2, 0, Some(2), "none")));
}
"#;

#[test]
fn test_check_translation() {
    let db = ParserDB::parse::<PostgreSqlDialect>(SCHEMA).expect("Unable to parse the schema");
    let temp_dir = tempfile::tempdir().expect("Unable to create temporary directory");
    let workspace_path = temp_dir.path().join("synql_check_translation");

    let checks = ExternalCrate::new("checks")
        .expect("A valid crate name")
        .functions([
            ExternalFunction::new("is_lowercase", syn::parse_quote!(::checks::is_lowercase)),
            ExternalFunction::new("is_shorter", syn::parse_quote!(::checks::is_shorter)),
            ExternalFunction::new("is_positive", syn::parse_quote!(::checks::is_positive)),
            ExternalFunction::new("is_even", syn::parse_quote!(::checks::is_even)),
        ])
        .into();
    let synql: SynQL<ParserDB> = SynQL::new(&db, &workspace_path)
        .name("synql-check-translation")
        .external_crate(checks)
        .member(TomlDependency::new("checks").path("checks").expect("A valid path"))
        .member(TomlDependency::new("assertions").path("assertions").expect("A valid path"))
        .generate_workspace_toml()
        .into();
    synql.generate().expect("Unable to generate workspace");

    for (path, contents) in [
        ("checks/Cargo.toml", CHECKS_MANIFEST),
        ("checks/src/lib.rs", CHECKS_LIB),
        ("assertions/Cargo.toml", ASSERTIONS_MANIFEST),
        ("assertions/tests/checks.rs", ASSERTIONS_TEST),
        ("assertions/src/lib.rs", "//! Assertions over the generated validation code.\n"),
    ] {
        let path = workspace_path.join(path);
        std::fs::create_dir_all(path.parent().expect("A parent directory"))
            .expect("Unable to create the directory");
        std::fs::write(&path, contents).expect("Unable to write the file");
    }

    let output = Command::new("cargo")
        .args(["test", "-p", "assertions"])
        .env("RUSTFLAGS", "-D warnings")
        .current_dir(&workspace_path)
        .output()
        .expect("Unable to run cargo test");
    assert!(
        output.status.success(),
        "The generated validation code failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
