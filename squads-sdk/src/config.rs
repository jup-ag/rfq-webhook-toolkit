use solana_sdk::pubkey::Pubkey;

use crate::error::{Result, SquadsSdkError};
use crate::pda::derive_vault_pda;
use crate::settings::SquadsSettings;

#[derive(Clone, Debug)]
pub struct SquadsWrapConfig {
    pub settings_pda: Pubkey,
    pub vault_pda: Pubkey,
    pub members: Vec<Pubkey>,
    pub threshold: u8,
}

impl SquadsWrapConfig {
    pub fn validate(&self) -> Result<()> {
        if self.members.is_empty() {
            return Err(SquadsSdkError::InvalidConfig(
                "members cannot be empty".into(),
            ));
        }
        if self.threshold == 0 {
            return Err(SquadsSdkError::InvalidConfig(
                "threshold must be greater than zero".into(),
            ));
        }
        if usize::from(self.threshold) > self.members.len() {
            return Err(SquadsSdkError::InvalidConfig(
                "threshold cannot be greater than members length".into(),
            ));
        }
        Ok(())
    }

    /// Build a [`SquadsWrapConfig`] from parsed on-chain settings.
    ///
    /// `signer_pubkeys` is the ordered list of members that will sign this
    /// transaction. The first entry becomes the fee payer. Every pubkey must
    /// be a member of the multisig and the count must meet the threshold.
    pub fn from_settings(
        settings: &SquadsSettings,
        settings_pda: Pubkey,
        vault_index: u8,
        signer_pubkeys: &[Pubkey],
    ) -> Result<Self> {
        if signer_pubkeys.is_empty() {
            return Err(SquadsSdkError::InvalidConfig(
                "signer_pubkeys cannot be empty".into(),
            ));
        }
        if (signer_pubkeys.len() as u16) < settings.threshold {
            return Err(SquadsSdkError::InvalidConfig(format!(
                "need at least {} signers (threshold), got {}",
                settings.threshold,
                signer_pubkeys.len()
            )));
        }
        for signer in signer_pubkeys {
            if !settings.members.iter().any(|m| m.pubkey == *signer) {
                return Err(SquadsSdkError::InvalidConfig(format!(
                    "signer {} is not a member of the multisig",
                    signer
                )));
            }
        }

        let (vault_pda, _) = derive_vault_pda(&settings_pda, vault_index);

        let config = Self {
            settings_pda,
            vault_pda,
            members: signer_pubkeys.to_vec(),
            threshold: settings.threshold as u8,
        };
        config.validate()?;
        Ok(config)
    }
}

/// Configurable options for transaction wrapping.
#[derive(Clone, Debug)]
pub struct WrapOptions {
    /// Multiplier applied to the original compute-unit limit.
    /// Squads CPI adds significant overhead; without simulation we use a
    /// conservative multiplier. Default: `2`.
    pub cu_multiplier: u32,
    /// Absolute cap for the compute-unit limit. Default: `1_400_000`.
    pub cu_cap: u32,
    /// Maximum serialized transaction size in bytes. Default: `1232` (Solana limit).
    pub tx_size_limit: usize,
}

impl Default for WrapOptions {
    fn default() -> Self {
        Self {
            cu_multiplier: 2,
            cu_cap: 1_400_000,
            tx_size_limit: 1232,
        }
    }
}
