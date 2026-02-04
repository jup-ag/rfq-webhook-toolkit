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

        let signature_failed =
            SwapState::RejectedWithReason(RejectionReason::SignatureVerificationFailed);
        let signature_failed_json = serde_json::to_string(&signature_failed).unwrap();
        println!(
            "SignatureVerificationFailed serialized: {}",
            signature_failed_json
        );

        // Test deserialization
        let deserialized_insufficient: SwapState =
            serde_json::from_str(&insufficient_balance_json).unwrap();
        assert_eq!(deserialized_insufficient, insufficient_balance);

        let deserialized_signature: SwapState =
            serde_json::from_str(&signature_failed_json).unwrap();
        assert_eq!(deserialized_signature, signature_failed);
    }

    #[test]
    fn test_rejection_reason_serialization() {
        let insufficient_balance = RejectionReason::InsufficientBalance;
        assert_eq!(
            serde_json::to_string(&insufficient_balance).unwrap(),
            "\"insufficientBalance\""
        );

        let signature_failed = RejectionReason::SignatureVerificationFailed;
        assert_eq!(
            serde_json::to_string(&signature_failed).unwrap(),
            "\"signatureVerificationFailed\""
        );
    }
}
