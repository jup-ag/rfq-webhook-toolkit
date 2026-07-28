use anyhow::{bail, Result};

/// Splits an 8-byte discriminator off the front, returning `(disc, remaining_bytes)`.
#[allow(dead_code)]
pub fn split_disc_and_bytes(bytes: &[u8]) -> Result<(&[u8; 8], &[u8])> {
    let Some((disc, remaining)) = bytes.split_first_chunk::<8>() else {
        bail!("Not enough bytes to split disc and bytes");
    };
    Ok((disc, remaining))
}

/// Splits a 1-byte discriminator off the front, returning `(disc, remaining_bytes)`.
#[allow(dead_code)]
pub fn split_disc1byte_and_bytes(bytes: &[u8]) -> Result<(&[u8; 1], &[u8])> {
    let Some((disc, remaining)) = bytes.split_first_chunk::<1>() else {
        bail!("Not enough bytes to split disc and bytes");
    };
    Ok((disc, remaining))
}
