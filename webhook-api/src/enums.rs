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
    #[default]
    InsufficientBalance,
    SignatureVerificationFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, Display, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum SwapState {
    Accepted,
    Rejected,
    RejectedWithReason(RejectionReason), // EXPERIMENTAL. NOT PROD READY
}
