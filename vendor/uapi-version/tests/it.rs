//! Test cases from systemd's src/test/test-string-util.c

use std::cmp::Ordering;

use uapi_version::{strverscmp, Version};

fn assert_ordering(a: &str, b: &str, expected: Ordering) {
    let ordering = strverscmp(a, b);
    assert_eq!(ordering, expected);
}

fn assert_ordering_list(versions: &[(&str, &str, Ordering)]) {
    for (a, b, expected) in versions {
        assert_ordering(a, b, *expected);
    }
}

fn assert_smaller(smaller: &str, bigger: &str) {
    assert_ordering(smaller, bigger, Ordering::Less);
}

fn assert_smaller_list(versions: &[(&str, &str)]) {
    for (a, b) in versions {
        assert_smaller(a, b);
    }
}

#[test]
fn constructing() {
    let _ = Version::from(String::from("a"));
    let _ = Version::from(&String::from("a"));
    let _ = Version::from("a");
}

#[test]
fn order() {
    let versions = [
        "~1",
        "",
        "ab",
        "abb",
        "abc",
        "0.0.1",
        "0.1.0",
        "0.10.0",
        "1.0.0",
        "0001",
        "002",
        "10.0.0",
        "12",
        "122",
        "122.9",
        "123~rc1",
        "123",
        "123-a",
        "123-a.1",
        "123-a1",
        "123-a1.1",
        "123-3",
        "123-3.1",
        "123^patch1",
        "123^1",
        "123.a-1",
        "123.1-1",
        "123a-1",
        "124",
    ];

    for (i, first) in versions.iter().enumerate() {
        for next in versions.iter().skip(i + 1) {
            assert_smaller(first, next);
        }
    }
}

#[test]
fn compare() {
    assert_smaller_list(&[
        ("123.45-67.88", "123.45-67.89"),
        ("123.45-67.89", "123.45-67.89a"),
        ("123.45-67.ab", "123.45-67.89"),
        ("123.45-67.9", "123.45-67.89"),
        ("123.45-67", "123.45-67.89"),
        ("123.45-66.89", "123.45-67.89"),
        ("123.45-9.99", "123.45-67.89"),
        ("123.42-99.99", "123.45-67.89"),
        ("123-99.99", "123.45-67.89"),
        ("245", "270"),
    ]);
}

#[test]
fn pre_releases() {
    assert_smaller_list(&[
        ("123~rc1-99.99", "123.45-67.89"),
        ("123~rc1-99.99", "123-45.67.89"),
        ("123~rc1-99.99", "123~rc2-67.89"),
        ("123~rc1-99.99", "123^aa2-67.89"),
        ("123~rc1-99.99", "123aa2-67.89"),
        ("1.0.0~rc1", "1.0.0"),
    ]);
}

#[test]
fn separator_between_version_and_release() {
    assert_smaller_list(&[
        ("123-99.99", "123.45-67.89"),
        ("123-99.99", "123^aa2-67.89"),
        ("123-99.99", "123aa2-67.89"),
    ]);
}

#[test]
fn patch_releases() {
    assert_smaller_list(&[
        ("123^45-67.89", "123.45-67.89"),
        ("123^aa1-99.99", "123^aa2-67.89"),
        ("123^aa2-67.89", "123aa2-67.89"),
    ]);
}

#[test]
fn point_releases() {
    assert_smaller_list(&[
        ("123.aa2-67.89", "123aa2-67.89"),
        ("123.aa2-67.89", "123.ab2-67.89"),
    ]);
}

#[test]
fn invalid_chars() {
    assert_ordering("123_aa2-67.89", "123aa+2-67.89", Ordering::Equal);
}

#[test]
fn corner_cases() {
    assert_ordering_list(&[
        ("123.", "123", Ordering::Greater),
        ("12_3", "123", Ordering::Less),
        ("12_3", "12", Ordering::Greater),
        ("12_3", "12.3", Ordering::Greater),
        ("123.0", "123", Ordering::Greater),
        ("123_0", "123", Ordering::Greater),
        ("123..0", "123.0", Ordering::Less),
    ]);
}

#[test]
fn empty_strings() {
    assert_ordering_list(&[
        ("0_", "0", Ordering::Equal),
        ("_0_", "0", Ordering::Equal),
        ("_0", "0", Ordering::Equal),
        ("0", "0___", Ordering::Equal),
        ("", "_", Ordering::Equal),
        ("_", "", Ordering::Equal),
        ("_", "_", Ordering::Equal),
        ("", "~", Ordering::Greater),
        ("~", "", Ordering::Less),
        ("~", "~", Ordering::Equal),
    ]);
}

#[test]
fn non_ascii() {
    assert_ordering_list(&[
        ("1٠١٢٣٤٥٦٧٨٩", "1", Ordering::Equal),
        ("1๐๑๒๓๔๕๖๗๘๙", "1", Ordering::Equal),
    ]);
}

#[test]
fn zeros() {
    assert_smaller_list(&[
        ("0.0.1", "0.0.2"),
        ("0.0.2", "0.2.0"),
        ("1.0.0", "1.0.1"),
        ("1.0.0", "1.1.0"),
        ("1.0.0", "1.1.1"),
        ("1.0.0", "10.0.0"),
        ("0.10.0", "0.100.0"),
        ("0.0.1", "0.0.10"),
        ("0.1.2", "0.10.2"),
        ("0.0.9", "1.0.0"),
    ]);
}
