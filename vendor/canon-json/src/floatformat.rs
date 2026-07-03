//
//  Copyright 2006-2019 WebPKI.org (http://webpki.org).
//  Copyright 2024 Gemini
//
//  Licensed under the Apache License, Version 2.0 (the "License");
//  you may not use this file except in compliance with the License.
//  You may in obtain a copy of the License at
//
//      https://www.apache.org/licenses/LICENSE-2.0
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
//

//! # JSON Canonicalizer - Number Formatter
//!
//! This library converts numbers in IEEE-754 double precision (`f64`) into the
//! canonical string format specified for JSON in EcmaScript Version 6 and forward.
//!
//! The primary application for this is canonicalization, such as the
//! [JSON Canonicalization Scheme (JCS)](https://tools.ietf.org/html/draft-rundgren-json-canonicalization-scheme-02).

use thiserror::Error;

// A constant bitmask to identify NaN and Infinity in an IEEE-754 f64.
// If the exponent bits are all set, the number is non-finite.
const INVALID_PATTERN: u64 = 0x7ff0_0000_0000_0000;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum NumberSerializationError {
    #[error("Unserializable number")]
    /// This number is a NaN or Infinity
    Unserializable,
}

/// Converts an `f64` floating-point number into its canonical JSON string representation.
///
/// This function adheres to the ES6 specification for representing numbers, which is
/// a requirement for JCS.
pub(crate) fn number_to_json(ieee_f64: f64) -> Result<String, NumberSerializationError> {
    // By converting the f64 to its raw u64 bits, we can easily check for
    // non-finite values (NaN and Infinity).
    let ieee_u64 = ieee_f64.to_bits();

    // Special case: NaN and Infinity are invalid in JSON.
    // The JCS specification mandates that implementations must reject them.
    if (ieee_u64 & INVALID_PATTERN) == INVALID_PATTERN {
        return Err(NumberSerializationError::Unserializable);
    }

    // Special case: Eliminate "-0" as mandated by the ES6/JCS specifications.
    // This comparison correctly handles both +0.0 and -0.0.
    if ieee_f64 == 0.0 {
        return Ok("0".to_string());
    }

    // Deal with the sign separately. We will format the absolute value of the number
    // and prepend the sign at the end.
    let (sign, num) = if ieee_f64.is_sign_negative() {
        ("-", -ieee_f64)
    } else {
        ("", ieee_f64)
    };

    // ES6 defines a specific range where numbers should be rendered in fixed-point
    // format ('f'), and scientific notation ('e') otherwise.
    let es6_formatted = if (1e-6..1e21).contains(&num) {
        // For numbers within this range, use fixed-point notation.
        // Rust's default `to_string` for floats is suitable as it doesn't produce
        // unnecessary trailing zeros (e.g., 25.0 becomes "25").
        num.to_string()
    } else {
        // For numbers outside the range, use scientific notation.
        // Rust's `format!("{:e}")` produces output like "1.23e9" or "1.23e-7".
        // The ES6/JCS standard requires a sign for the exponent, e.g., "1.23e+9".
        // We need to manually insert the '+' for positive exponents.
        let mut scientific = format!("{:e}", num);
        if let Some(e_pos) = scientific.find('e') {
            let next_char_pos = e_pos + 1;
            // Check if the character after 'e' is a digit (meaning a positive exponent).
            if scientific
                .chars()
                .nth(next_char_pos)
                .is_some_and(|c| c.is_ascii_digit())
            {
                scientific.insert(next_char_pos, '+');
            }
        }
        scientific
    };

    // Combine the sign and the formatted number for the final result.
    Ok(format!("{}{}", sign, es6_formatted))
}

// Unit tests to verify correctness against the specification.
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64;

    #[test]
    fn test_zero() {
        assert_eq!(number_to_json(0.0).unwrap(), "0");
    }

    #[test]
    fn test_negative_zero() {
        assert_eq!(number_to_json(-0.0).unwrap(), "0");
    }

    #[test]
    fn test_simple_integer() {
        assert_eq!(number_to_json(25.0).unwrap(), "25");
    }

    #[test]
    fn test_negative_integer() {
        assert_eq!(number_to_json(-50.0).unwrap(), "-50");
    }

    #[test]
    fn test_simple_float() {
        assert_eq!(number_to_json(2.5).unwrap(), "2.5");
    }

    #[test]
    fn test_negative_float() {
        assert_eq!(number_to_json(-3.5).unwrap(), "-3.5");
    }

    #[test]
    fn test_fixed_point_boundary_small() {
        assert_eq!(number_to_json(1e-6).unwrap(), "0.000001");
    }

    #[test]
    fn test_fixed_point_boundary_large() {
        // 1e21 is the exclusive upper bound, so it should be scientific.
        assert_eq!(number_to_json(1e21).unwrap(), "1e+21");
        // A value just under 1e21.
        assert_eq!(
            number_to_json(9.999999999999999e20).unwrap(),
            "999999999999999900000"
        );
    }

    #[test]
    fn test_scientific_positive_exponent() {
        assert_eq!(number_to_json(1.23e22).unwrap(), "1.23e+22");
    }

    #[test]
    fn test_scientific_negative_exponent() {
        assert_eq!(number_to_json(1.23e-7).unwrap(), "1.23e-7");
    }

    #[test]
    fn test_scientific_no_plus_needed() {
        let n = -1.23e-7;
        assert_eq!(number_to_json(n).unwrap(), "-1.23e-7");
    }

    #[test]
    fn test_scientific_plus_insertion() {
        let n = 1e9;
        assert_eq!(number_to_json(n).unwrap(), "1000000000");
    }

    #[test]
    fn test_scientific_plus_insertion_negative_number() {
        let n = -1e12;
        assert_eq!(number_to_json(n).unwrap(), "-1000000000000");
    }

    #[test]
    fn test_invalid_numbers() {
        assert!(number_to_json(f64::NAN).is_err());
        assert!(number_to_json(f64::INFINITY).is_err());
        assert!(number_to_json(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn test_error_message() {
        #[allow(unreachable_patterns)]
        match number_to_json(f64::NAN).unwrap_err() {
            NumberSerializationError::Unserializable => {}
            _ => unreachable!(),
        }
    }
}
