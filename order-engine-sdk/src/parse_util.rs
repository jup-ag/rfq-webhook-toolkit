use std::fmt;

/// Errors produced while splitting a discriminator off an instruction data buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// The buffer was shorter than the discriminator it had to yield.
    NotEnoughBytes { expected: usize, actual: usize },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotEnoughBytes { expected, actual } => write!(
                f,
                "Not enough bytes to split disc and bytes: needed {expected}, got {actual}"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

/// Splits an 8-byte discriminator (anchor style) off the front, returning `(disc, remaining_bytes)`.
pub fn split_disc_and_bytes(bytes: &[u8]) -> Result<(&[u8; 8], &[u8]), ParseError> {
    bytes
        .split_first_chunk::<8>()
        .ok_or(ParseError::NotEnoughBytes {
            expected: 8,
            actual: bytes.len(),
        })
}

/// Splits a 1-byte discriminator off the front, returning `(disc, remaining_bytes)`.
pub fn split_disc1byte_and_bytes(bytes: &[u8]) -> Result<(&[u8; 1], &[u8]), ParseError> {
    bytes
        .split_first_chunk::<1>()
        .ok_or(ParseError::NotEnoughBytes {
            expected: 1,
            actual: bytes.len(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn too_short(expected: usize, actual: usize) -> String {
        format!("Not enough bytes to split disc and bytes: needed {expected}, got {actual}")
    }

    #[test]
    fn test_split_disc_and_bytes_exact_length_leaves_no_remainder() {
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8];
        let (disc, remaining) = split_disc_and_bytes(&bytes).unwrap();
        assert_eq!(disc, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(remaining, &[] as &[u8]);
    }

    #[test]
    fn test_split_disc_and_bytes_splits_at_the_eighth_byte() {
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let (disc, remaining) = split_disc_and_bytes(&bytes).unwrap();
        assert_eq!(disc, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(remaining, &[9, 10, 11]);
    }

    #[test]
    fn test_split_disc_and_bytes_rejects_fewer_than_eight_bytes() {
        for len in 0..8usize {
            let bytes = vec![0xAAu8; len];
            let err = split_disc_and_bytes(&bytes)
                .expect_err("{len} bytes must not yield an 8-byte discriminator");
            assert_eq!(
                err.to_string(),
                too_short(8, len),
                "wrong error for length {len}"
            );
        }
    }

    #[test]
    fn test_split_disc1byte_and_bytes_exact_length_leaves_no_remainder() {
        let bytes = [7];
        let (disc, remaining) = split_disc1byte_and_bytes(&bytes).unwrap();
        assert_eq!(disc, &[7]);
        assert_eq!(remaining, &[] as &[u8]);
    }

    #[test]
    fn test_split_disc1byte_and_bytes_splits_at_the_first_byte() {
        let bytes = [7, 8, 9];
        let (disc, remaining) = split_disc1byte_and_bytes(&bytes).unwrap();
        assert_eq!(disc, &[7]);
        assert_eq!(remaining, &[8, 9]);
    }

    #[test]
    fn test_split_disc1byte_and_bytes_rejects_empty_input() {
        let err = split_disc1byte_and_bytes(&[]).expect_err("empty input has no discriminator");
        assert_eq!(err.to_string(), too_short(1, 0));
    }

    /// The error is structured, so callers can inspect the byte counts instead
    /// of parsing the message.
    #[test]
    fn test_error_reports_expected_and_actual_byte_counts() {
        assert_eq!(
            split_disc_and_bytes(&[1, 2, 3]).unwrap_err(),
            ParseError::NotEnoughBytes {
                expected: 8,
                actual: 3
            }
        );
        assert_eq!(
            split_disc1byte_and_bytes(&[]).unwrap_err(),
            ParseError::NotEnoughBytes {
                expected: 1,
                actual: 0
            }
        );
    }

    /// `fill.rs` calls `.context(..)` on these results, which relies on anyhow's
    /// blanket impl for `Result<T, E> where E: StdError + Send + Sync + 'static`.
    /// If `ParseError` ever loses that bound, those call sites break — fail here instead.
    #[test]
    fn test_parse_error_satisfies_anyhow_context_bound() {
        fn assert_bound<E: std::error::Error + Send + Sync + 'static>() {}
        assert_bound::<ParseError>();
    }

    /// A 1-byte discriminator must not be read out of an 8-byte-discriminator
    /// buffer or vice versa — the two helpers are not interchangeable.
    #[test]
    fn test_the_two_helpers_disagree_on_a_single_byte_input() {
        let bytes = [5];
        assert!(split_disc_and_bytes(&bytes).is_err());
        assert_eq!(split_disc1byte_and_bytes(&bytes).unwrap().0, &[5]);
    }

    /// Both helpers must borrow from the caller's buffer rather than copy it,
    /// so callers can keep deserializing straight out of `remaining`.
    #[test]
    fn test_returned_slices_borrow_from_the_input_buffer() {
        let bytes = [1u8, 2, 3, 4, 5, 6, 7, 8, 9];

        let (disc, remaining) = split_disc_and_bytes(&bytes).unwrap();
        assert_eq!(disc.as_ptr(), bytes.as_ptr());
        assert_eq!(remaining.as_ptr(), bytes[8..].as_ptr());

        let (disc1, remaining1) = split_disc1byte_and_bytes(&bytes).unwrap();
        assert_eq!(disc1.as_ptr(), bytes.as_ptr());
        assert_eq!(remaining1.as_ptr(), bytes[1..].as_ptr());
    }
}
