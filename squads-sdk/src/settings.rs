use solana_sdk::pubkey::Pubkey;

use crate::error::{Result, SquadsSdkError};

/// Bitmask for a Squads member's permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberPermissions(pub u8);

impl MemberPermissions {
    pub const INITIATE: u8 = 1 << 0;
    pub const VOTE: u8 = 1 << 1;
    pub const EXECUTE: u8 = 1 << 2;

    pub fn can_initiate(self) -> bool {
        self.0 & Self::INITIATE != 0
    }

    pub fn can_vote(self) -> bool {
        self.0 & Self::VOTE != 0
    }

    pub fn can_execute(self) -> bool {
        self.0 & Self::EXECUTE != 0
    }
}

/// A parsed member from a Squads V5 settings account.
#[derive(Debug, Clone)]
pub struct SquadsMember {
    pub pubkey: Pubkey,
    pub permissions: MemberPermissions,
}

/// Parsed Squads V5 settings account (SmartAccount).
#[derive(Debug, Clone)]
pub struct SquadsSettings {
    pub multisig: Pubkey,
    pub settings_index: u64,
    pub stale_transaction_index: u64,
    pub threshold: u16,
    pub time_lock: u32,
    pub transaction_index: u64,
    pub archival_authority: Option<Pubkey>,
    pub archivable_after: u64,
    pub bump: u8,
    pub members: Vec<SquadsMember>,
}

/// Expected Anchor discriminator for `Settings` (Squads V5).
fn settings_discriminator() -> [u8; 8] {
    crate::get_discriminator_bytes(b"account:Settings")
}

fn read_u8(data: &[u8], offset: &mut usize) -> Result<u8> {
    if *offset >= data.len() {
        return Err(SquadsSdkError::InvalidSettingsData(
            "unexpected end of data".into(),
        ));
    }
    let val = data[*offset];
    *offset += 1;
    Ok(val)
}

fn read_u16_le(data: &[u8], offset: &mut usize) -> Result<u16> {
    if *offset + 2 > data.len() {
        return Err(SquadsSdkError::InvalidSettingsData(
            "unexpected end of data".into(),
        ));
    }
    let val = u16::from_le_bytes([data[*offset], data[*offset + 1]]);
    *offset += 2;
    Ok(val)
}

fn read_u32_le(data: &[u8], offset: &mut usize) -> Result<u32> {
    if *offset + 4 > data.len() {
        return Err(SquadsSdkError::InvalidSettingsData(
            "unexpected end of data".into(),
        ));
    }
    let val = u32::from_le_bytes(data[*offset..*offset + 4].try_into().unwrap());
    *offset += 4;
    Ok(val)
}

fn read_u64_le(data: &[u8], offset: &mut usize) -> Result<u64> {
    if *offset + 8 > data.len() {
        return Err(SquadsSdkError::InvalidSettingsData(
            "unexpected end of data".into(),
        ));
    }
    let val = u64::from_le_bytes(data[*offset..*offset + 8].try_into().unwrap());
    *offset += 8;
    Ok(val)
}

fn read_pubkey(data: &[u8], offset: &mut usize) -> Result<Pubkey> {
    if *offset + 32 > data.len() {
        return Err(SquadsSdkError::InvalidSettingsData(
            "unexpected end of data".into(),
        ));
    }
    let key = Pubkey::new_from_array(data[*offset..*offset + 32].try_into().unwrap());
    *offset += 32;
    Ok(key)
}

/// Parse raw Squads V5 settings account data into [`SquadsSettings`].
///
/// The caller is responsible for fetching the account via RPC.
/// This function validates the Anchor discriminator and extracts all fields.
pub fn parse_squads_settings(data: &[u8]) -> Result<SquadsSettings> {
    if data.len() < 78 {
        return Err(SquadsSdkError::InvalidSettingsData(format!(
            "account data too short: {} bytes, need at least 78",
            data.len()
        )));
    }

    // Validate discriminator
    let expected_disc = settings_discriminator();
    if data[0..8] != expected_disc {
        return Err(SquadsSdkError::InvalidSettingsData(format!(
            "wrong discriminator: expected {:?}, got {:?}",
            expected_disc,
            &data[0..8]
        )));
    }

    let mut offset = 8;

    let multisig = read_pubkey(data, &mut offset)?;
    let settings_index = read_u64_le(data, &mut offset)?;
    let stale_transaction_index = read_u64_le(data, &mut offset)?;
    let threshold = read_u16_le(data, &mut offset)?;
    let time_lock = read_u32_le(data, &mut offset)?;
    let transaction_index = read_u64_le(data, &mut offset)?;

    // Skip 8 bytes padding
    if offset + 8 > data.len() {
        return Err(SquadsSdkError::InvalidSettingsData(
            "unexpected end of data at padding".into(),
        ));
    }
    offset += 8;

    // archival_authority: COption<Pubkey>
    let archival_tag = read_u8(data, &mut offset)?;
    let archival_authority = if archival_tag == 1 {
        Some(read_pubkey(data, &mut offset)?)
    } else {
        None
    };

    let archivable_after = read_u64_le(data, &mut offset)?;
    let bump = read_u8(data, &mut offset)?;

    // signers: Vec<SquadsMember>
    let signers_len = read_u32_le(data, &mut offset)? as usize;
    let mut members = Vec::with_capacity(signers_len);

    for _ in 0..signers_len {
        let pubkey = read_pubkey(data, &mut offset)?;
        let permissions = MemberPermissions(read_u8(data, &mut offset)?);
        members.push(SquadsMember {
            pubkey,
            permissions,
        });
    }

    Ok(SquadsSettings {
        multisig,
        settings_index,
        stale_transaction_index,
        threshold,
        time_lock,
        transaction_index,
        archival_authority,
        archivable_after,
        bump,
        members,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic V5 settings account buffer.
    fn build_settings_buffer(
        multisig: &Pubkey,
        threshold: u16,
        time_lock: u32,
        archival_authority: Option<&Pubkey>,
        bump: u8,
        members: &[(Pubkey, u8)],
    ) -> Vec<u8> {
        let mut buf = Vec::new();

        // Discriminator
        buf.extend_from_slice(&settings_discriminator());
        // multisig
        buf.extend_from_slice(multisig.as_ref());
        // settings_index
        buf.extend_from_slice(&0u64.to_le_bytes());
        // stale_transaction_index
        buf.extend_from_slice(&0u64.to_le_bytes());
        // threshold
        buf.extend_from_slice(&threshold.to_le_bytes());
        // time_lock
        buf.extend_from_slice(&time_lock.to_le_bytes());
        // transaction_index
        buf.extend_from_slice(&42u64.to_le_bytes());
        // padding
        buf.extend_from_slice(&[0u8; 8]);
        // archival_authority
        match archival_authority {
            Some(key) => {
                buf.push(1);
                buf.extend_from_slice(key.as_ref());
            }
            None => {
                buf.push(0);
            }
        }
        // archivable_after
        buf.extend_from_slice(&0u64.to_le_bytes());
        // bump
        buf.push(bump);
        // members vec
        buf.extend_from_slice(&(members.len() as u32).to_le_bytes());
        for (key, perms) in members {
            buf.extend_from_slice(key.as_ref());
            buf.push(*perms);
        }

        buf
    }

    #[test]
    fn parses_settings_without_archival_authority() {
        let multisig = Pubkey::new_unique();
        let member_a = Pubkey::new_unique();
        let member_b = Pubkey::new_unique();

        let data = build_settings_buffer(
            &multisig,
            2,
            0,
            None,
            254,
            &[(member_a, 0x07), (member_b, 0x07)],
        );

        let settings = parse_squads_settings(&data).unwrap();
        assert_eq!(settings.multisig, multisig);
        assert_eq!(settings.threshold, 2);
        assert_eq!(settings.time_lock, 0);
        assert_eq!(settings.transaction_index, 42);
        assert!(settings.archival_authority.is_none());
        assert_eq!(settings.bump, 254);
        assert_eq!(settings.members.len(), 2);
        assert_eq!(settings.members[0].pubkey, member_a);
        assert_eq!(settings.members[0].permissions.0, 0x07);
        assert!(settings.members[0].permissions.can_initiate());
        assert!(settings.members[0].permissions.can_vote());
        assert!(settings.members[0].permissions.can_execute());
        assert_eq!(settings.members[1].pubkey, member_b);
    }

    #[test]
    fn parses_settings_with_archival_authority() {
        let multisig = Pubkey::new_unique();
        let archival = Pubkey::new_unique();
        let member = Pubkey::new_unique();

        let data =
            build_settings_buffer(&multisig, 1, 3600, Some(&archival), 253, &[(member, 0x03)]);

        let settings = parse_squads_settings(&data).unwrap();
        assert_eq!(settings.threshold, 1);
        assert_eq!(settings.time_lock, 3600);
        assert_eq!(settings.archival_authority, Some(archival));
        assert_eq!(settings.bump, 253);
        assert_eq!(settings.members.len(), 1);
        assert!(settings.members[0].permissions.can_initiate());
        assert!(settings.members[0].permissions.can_vote());
        assert!(!settings.members[0].permissions.can_execute());
    }

    #[test]
    fn rejects_wrong_discriminator() {
        let mut data = build_settings_buffer(
            &Pubkey::new_unique(),
            1,
            0,
            None,
            255,
            &[(Pubkey::new_unique(), 0x07)],
        );
        // Corrupt discriminator
        data[0] = 0xFF;

        let err = parse_squads_settings(&data).unwrap_err();
        assert!(err.to_string().contains("wrong discriminator"));
    }

    #[test]
    fn rejects_truncated_data() {
        // Too short for minimum header
        let err = parse_squads_settings(&[0u8; 10]).unwrap_err();
        assert!(err.to_string().contains("too short"));

        // Valid discriminator, passes length check, but truncated at signers vec
        let mut data = build_settings_buffer(
            &Pubkey::new_unique(),
            1,
            0,
            None,
            255,
            &[(Pubkey::new_unique(), 0x07)],
        );
        // Chop off the last member bytes so the member parsing fails
        data.truncate(data.len() - 10);
        let err = parse_squads_settings(&data).unwrap_err();
        assert!(err.to_string().contains("unexpected end of data"));
    }

    #[test]
    fn rejects_data_too_short() {
        let err = parse_squads_settings(&[0u8; 4]).unwrap_err();
        assert!(err.to_string().contains("too short"));
    }
}
