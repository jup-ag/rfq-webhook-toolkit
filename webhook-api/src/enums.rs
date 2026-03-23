use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use utoipa::ToSchema;

#[derive(
    Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, EnumString, Display,
)]
#[serde(rename_all = "camelCase")]
pub enum QuoteType {
    #[serde(alias = "exactIn", alias = "exact_in", alias = "ExactIn")]
    ExactIn,
    #[serde(alias = "exactOut", alias = "exact_out", alias = "ExactOut")]
    ExactOut,
}

#[derive(
    Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, EnumString, Display,
)]
#[serde(rename_all = "camelCase")]
pub enum Protocol {
    V1,
}

#[derive(
    Debug,
    Copy,
    Clone,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    ToSchema,
    Display,
    EnumString,
    Default,
)]
#[serde(rename_all = "camelCase")]
pub enum RejectionReason {
    InsufficientBalance,
    InsufficientBalanceForAtaCreation,
    SignatureVerificationFailed,
    #[default]
    BotActivityDetected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, Display, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum SwapState {
    Accepted,
    Rejected,
    RejectedWithReason(RejectionReason),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_swap_state_serialization() {
        // Test basic states
        let accepted = SwapState::Accepted;
        assert_eq!(serde_json::to_string(&accepted).unwrap(), "\"accepted\"");

        let rejected = SwapState::Rejected;
        assert_eq!(serde_json::to_string(&rejected).unwrap(), "\"rejected\"");

        // Test RejectedWithReason states
        let insufficient_balance =
            SwapState::RejectedWithReason(RejectionReason::InsufficientBalance);
        let insufficient_balance_json = serde_json::to_string(&insufficient_balance).unwrap();
        println!(
            "InsufficientBalance serialized: {}",
            insufficient_balance_json
        );

        let insufficient_balance_ata =
            SwapState::RejectedWithReason(RejectionReason::InsufficientBalanceForAtaCreation);
        let insufficient_balance_ata_json = serde_json::to_string(&insufficient_balance_ata).unwrap();
        println!(
            "InsufficientBalanceForAtaCreation serialized: {}",
            insufficient_balance_ata_json
        );

        let signature_failed =
            SwapState::RejectedWithReason(RejectionReason::SignatureVerificationFailed);
        let signature_failed_json = serde_json::to_string(&signature_failed).unwrap();
        println!(
            "SignatureVerificationFailed serialized: {}",
            signature_failed_json
        );

        let bot_activity =
            SwapState::RejectedWithReason(RejectionReason::BotActivityDetected);
        let bot_activity_json = serde_json::to_string(&bot_activity).unwrap();
        println!(
            "BotActivityDetected serialized: {}",
            bot_activity_json
        );

        // Test deserialization
        let deserialized_insufficient: SwapState =
            serde_json::from_str(&insufficient_balance_json).unwrap();
        assert_eq!(deserialized_insufficient, insufficient_balance);

        let deserialized_insufficient_ata: SwapState =
            serde_json::from_str(&insufficient_balance_ata_json).unwrap();
        assert_eq!(deserialized_insufficient_ata, insufficient_balance_ata);

        let deserialized_signature: SwapState =
            serde_json::from_str(&signature_failed_json).unwrap();
        assert_eq!(deserialized_signature, signature_failed);

        let deserialized_bot_activity: SwapState =
            serde_json::from_str(&bot_activity_json).unwrap();
        assert_eq!(deserialized_bot_activity, bot_activity);
    }

    #[test]
    fn test_rejection_reason_serialization() {
        let insufficient_balance = RejectionReason::InsufficientBalance;
        assert_eq!(
            serde_json::to_string(&insufficient_balance).unwrap(),
            "\"insufficientBalance\""
        );

        let insufficient_balance_ata = RejectionReason::InsufficientBalanceForAtaCreation;
        assert_eq!(
            serde_json::to_string(&insufficient_balance_ata).unwrap(),
            "\"insufficientBalanceForAtaCreation\""
        );

        let signature_failed = RejectionReason::SignatureVerificationFailed;
        assert_eq!(
            serde_json::to_string(&signature_failed).unwrap(),
            "\"signatureVerificationFailed\""
        );

        let bot_activity = RejectionReason::BotActivityDetected;
        assert_eq!(
            serde_json::to_string(&bot_activity).unwrap(),
            "\"botActivityDetected\""
        );
    }
}
