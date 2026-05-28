use solana_sdk::pubkey::Pubkey;

use crate::SQUADS_PROGRAM_ID;

/// Derive the settings PDA (smart account) from the create key.
///
/// Seeds: `["smart_account", create_key]`
pub fn derive_settings_pda(create_key: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"smart_account", create_key.as_ref()], &SQUADS_PROGRAM_ID)
}

/// Derive the vault PDA from the settings PDA and vault index.
///
/// Seeds: `["smart_account", settings_pda, "smart_account", vault_index]`
pub fn derive_vault_pda(settings_pda: &Pubkey, vault_index: u8) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"smart_account",
            settings_pda.as_ref(),
            b"smart_account",
            &[vault_index],
        ],
        &SQUADS_PROGRAM_ID,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_settings_pda_is_deterministic() {
        let create_key = Pubkey::new_unique();
        let (pda1, bump1) = derive_settings_pda(&create_key);
        let (pda2, bump2) = derive_settings_pda(&create_key);
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
    }

    #[test]
    fn derive_vault_pda_is_deterministic() {
        let settings = Pubkey::new_unique();
        let (pda1, bump1) = derive_vault_pda(&settings, 0);
        let (pda2, bump2) = derive_vault_pda(&settings, 0);
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
    }

    #[test]
    fn different_vault_indices_produce_different_addresses() {
        let settings = Pubkey::new_unique();
        let (vault0, _) = derive_vault_pda(&settings, 0);
        let (vault1, _) = derive_vault_pda(&settings, 1);
        let (vault2, _) = derive_vault_pda(&settings, 2);
        assert_ne!(vault0, vault1);
        assert_ne!(vault1, vault2);
        assert_ne!(vault0, vault2);
    }

    #[test]
    fn different_create_keys_produce_different_settings() {
        let key_a = Pubkey::new_unique();
        let key_b = Pubkey::new_unique();
        let (settings_a, _) = derive_settings_pda(&key_a);
        let (settings_b, _) = derive_settings_pda(&key_b);
        assert_ne!(settings_a, settings_b);
    }

    #[test]
    fn derive_vault_pda_matches_on_chain_program() {
        use solana_sdk::pubkey;
        // Known settings PDA → vault PDA pair from mainnet
        let settings_pda = pubkey!("8QJmMPTmRJSLsGxjAXYDquEtWjVaKvBK8HVvV4Mcn1gB");
        let expected_vault = pubkey!("HviMBVH4L84zW7xKL8oSPcDbXrjLVyRkCiYUjcVCVACE");
        let (vault, _) = derive_vault_pda(&settings_pda, 0);
        assert_eq!(
            vault, expected_vault,
            "vault PDA derivation must match the on-chain Squads program"
        );
    }
}
