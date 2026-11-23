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
    /// Your simulation of the signed transaction failed; put its error in `rejection_reason`.
    /// Jupiter re-simulates the fill where it can, and still counts the rejection against you
    /// when the fill simulates fine, its blockhash or expiry has passed by the time Jupiter
    /// checks, or the maker caused the failure. Use `InsufficientBalance` or
    /// `InsufficientBalanceForAtaCreation` when the taker is short.
    SimulationFailed,
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
        let insufficient_balance_ata_json =
            serde_json::to_string(&insufficient_balance_ata).unwrap();
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

        let bot_activity = SwapState::RejectedWithReason(RejectionReason::BotActivityDetected);
        let bot_activity_json = serde_json::to_string(&bot_activity).unwrap();
        println!("BotActivityDetected serialized: {}", bot_activity_json);

        let simulation_failed = SwapState::RejectedWithReason(RejectionReason::SimulationFailed);
        let simulation_failed_json = serde_json::to_string(&simulation_failed).unwrap();
        assert_eq!(
            simulation_failed_json,
            r#"{"rejectedWithReason":"simulationFailed"}"#
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

        let deserialized_simulation_failed: SwapState =
            serde_json::from_str(&simulation_failed_json).unwrap();
        assert_eq!(deserialized_simulation_failed, simulation_failed);
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

        let simulation_failed = RejectionReason::SimulationFailed;
        assert_eq!(
            serde_json::to_string(&simulation_failed).unwrap(),
            "\"simulationFailed\""
        );
    }
}

/// Which Ondo operation a flash-loan fill wraps around the order-engine fill.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum FlashSide {
    /// Taker buys the GM token: borrow USDC, mint, fill, repay.
    Mint,
    /// Taker sells the GM token: borrow USDC, fill, redeem, repay.
    Redeem,
}
