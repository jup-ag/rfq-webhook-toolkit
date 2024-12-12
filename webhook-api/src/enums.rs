use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum QuoteType {
    ExactIn,
    ExactOut,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum Protocol {
    V1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, Display, EnumString)]
#[serde(rename_all = "camelCase")]
pub enum SwapState {
    Accepted,
    Rejected,
}
