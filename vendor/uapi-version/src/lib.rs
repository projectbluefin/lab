//! Compare versions according to the [UAPI Version Format
//! Specification](https://uapi-group.org/specifications/specs/version_format_specification/).
//!
//! This implementation is written purely in Rust and does not rely on any third party
//! dependencies. Most notably, it doesn't link to `libsystemd`. It is `#![no_std]` and thus can,
//! for example, also be used for UEFI development.
//!
//! # Examples
//!
//! You can compare two versions:
//!
//! ```
//! use std::cmp::Ordering;
//!
//! use uapi_version::Version;
//!
//! let a = Version::from("225.1");
//! let b = Version::from("2");
//!
//! assert_eq!(a.cmp(&b), Ordering::Greater)
//! ```
//!
//! [`Version`] implements [`std::cmp::Ord`] and thus can be used to order a list of versions.
//!
//! ```
//! use uapi_version::Version;
//!
//! let mut versions = [
//!     "5.2",
//!     "abc-5",
//!     "1.0.0~rc1",
//! ].map(Version::from);
//!
//! versions.sort();
//!
//! assert_eq!(versions, [ "abc-5", "1.0.0~rc1", "5.2" ].map(Version::from))
//! ```
//!
//! You can also use [`strverscmp`] to compare two strings directly:
//!
//! ```
//! use std::cmp::Ordering;
//!
//! use uapi_version::strverscmp;
//!
//! assert_eq!(strverscmp("124", "123"), Ordering::Greater)
//! ```
#![no_std]

extern crate alloc;

use alloc::fmt;
use alloc::string::String;
use core::cmp::Ordering;

/// The `Version` type.
///
/// Can be built from any string that is a sequence of zero or more characters.
///
/// # Examples
///
/// ```
/// use std::cmp::Ordering;
///
/// use uapi_version::Version;
///
/// let a = Version::from("1.0.0");
/// let b = Version::from("2.0.0");
///
/// // `a` is smaller (i.e. older) than `b`.
/// assert_eq!(a.cmp(&b), Ordering::Less)
/// ```
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct Version(String);

impl Version {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl From<&str> for Version {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<String> for Version {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&String> for Version {
    fn from(s: &String) -> Self {
        Self(s.into())
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        strverscmp(&self.0, &other.0)
    }
}

/// Compare two version strings.
///
/// # Examples
///
/// ```
/// use std::cmp::Ordering;
///
/// use uapi_version::strverscmp;
///
/// assert_eq!(strverscmp("1.0.0", "2.0.0"), Ordering::Less)
/// ```
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn strverscmp(a: &str, b: &str) -> Ordering {
    let mut left_iter = a.chars().peekable();
    let mut right_iter = b.chars().peekable();

    loop {
        let mut left = left_iter.next();
        let mut right = right_iter.next();

        // Step 1: Skip invalid chars
        while left.is_some() && !left.is_some_and(is_valid_version_char) {
            left = left_iter.next();
        }
        while right.is_some() && !right.is_some_and(is_valid_version_char) {
            right = right_iter.next();
        }

        // Step 2: Handle '~'
        if left.is_some_and(|c| c == '~') || right.is_some_and(|c| c == '~') {
            let ordering = compare_special_char('~', left, right);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        // Step 3: Handle empty
        if left.is_none() || right.is_none() {
            return left.cmp(&right);
        }

        // Step 4: Handle '-'
        if left.is_some_and(|c| c == '-') || right.is_some_and(|c| c == '-') {
            let ordering = compare_special_char('-', left, right);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        // Step 5: Handle '^'
        if left.is_some_and(|c| c == '^') || right.is_some_and(|c| c == '^') {
            let ordering = compare_special_char('^', left, right);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        // Step 6: Handle '.'
        if left.is_some_and(|c| c == '.') || right.is_some_and(|c| c == '.') {
            let ordering = compare_special_char('.', left, right);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }

        // Step 7: Handle numerical prefix
        if left.is_some_and(|c| c.is_ascii_digit()) || right.is_some_and(|c| c.is_ascii_digit()) {
            // Skip leading '0's
            while left.is_some_and(|c| c == '0') {
                if !left_iter.peek().is_some_and(|c| c == &'0') {
                    break;
                }
                left = left_iter.next();
            }
            while right.is_some_and(|c| c == '0') {
                if !right_iter.peek().is_some_and(|c| c == &'0') {
                    break;
                }
                right = right_iter.next();
            }

            let mut left_digit_prefix = String::new();
            while left.is_some_and(|c| c.is_ascii_digit()) {
                if let Some(char) = left {
                    left_digit_prefix.push(char);
                }
                if !left_iter.peek().is_some_and(char::is_ascii_digit) {
                    break;
                }
                left = left_iter.next();
            }

            let mut right_digit_prefix = String::new();
            while right.is_some_and(|c| c.is_ascii_digit()) {
                if let Some(char) = right {
                    right_digit_prefix.push(char);
                }
                if !right_iter.peek().is_some_and(char::is_ascii_digit) {
                    break;
                }
                right = right_iter.next();
            }

            if left_digit_prefix.len() != right_digit_prefix.len() {
                return left_digit_prefix.len().cmp(&right_digit_prefix.len());
            }

            let ordering = left_digit_prefix.cmp(&right_digit_prefix);
            if ordering != Ordering::Equal {
                return ordering;
            }
        // Step 8: Handle alphabetical prefix
        } else {
            let mut left_alpha_prefix = String::new();
            while left.is_some_and(|c| c.is_ascii_alphabetic()) {
                if let Some(char) = left {
                    left_alpha_prefix.push(char);
                }
                if !left_iter.peek().is_some_and(char::is_ascii_alphabetic) {
                    break;
                }
                left = left_iter.next();
            }

            let mut right_alpha_prefix = String::new();
            while right.is_some_and(|c| c.is_ascii_alphabetic()) {
                if let Some(char) = right {
                    right_alpha_prefix.push(char);
                }
                if !right_iter.peek().is_some_and(char::is_ascii_alphabetic) {
                    break;
                }
                right = right_iter.next();
            }

            let ordering = left_alpha_prefix.cmp(&right_alpha_prefix);
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
    }
}

fn compare_special_char(char: char, left: Option<char>, right: Option<char>) -> Ordering {
    let left_bool = !left.is_some_and(|c| c == char);
    let right_bool = !right.is_some_and(|c| c == char);
    left_bool.cmp(&right_bool)
}

fn is_valid_version_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '~' | '-' | '^' | '.')
}
