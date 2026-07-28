use anyhow::{bail, Result};

/// Splits an 8-byte discriminator (anchor style) off the front, returning `(disc, remaining_bytes)`.
pub fn split_disc_and_bytes(bytes: &[u8]) -> Result<(&[u8; 8], &[u8])> {
    let Some((disc, remaining)) = bytes.split_first_chunk::<8>() else {
        bail!("Not enough bytes to split disc and bytes");
    };
    Ok((disc, remaining))
}

/// Splits a 1-byte discriminator off the front, returning `(disc, remaining_bytes)`.
pub fn split_disc1byte_and_bytes(bytes: &[u8]) -> Result<(&[u8; 1], &[u8])> {
    let Some((disc, remaining)) = bytes.split_first_chunk::<1>() else {
        bail!("Not enough bytes to split disc and bytes");
    };
    Ok((disc, remaining))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOO_SHORT: &str = "Not enough bytes to split disc and bytes";

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
            assert_eq!(err.to_string(), TOO_SHORT, "wrong error for length {len}");
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
        assert_eq!(err.to_string(), TOO_SHORT);
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
