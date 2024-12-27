use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::enums::{Protocol, QuoteType, SwapState};

/// Response to a quote request from the Market Maker
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    #[schema(examples("629bddf3-0038-43a6-8956-f5433d6b1191"))]
    pub request_id: String,
    #[schema(examples("59db3e19-c7b0-4753-a8aa-206701004498"))]
    pub quote_id: String,
    #[schema(examples("So11111111111111111111111111111111111111112"))]
    pub token_in: String,
    #[schema(examples("250000000"))]
    pub amount_in: String,
    #[schema(examples("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"))]
    pub token_out: String,
    #[schema(examples("ExactIn"))]
    pub quote_type: QuoteType,
    #[schema(examples("v1"))]
    pub protocol: Protocol,
    #[schema(examples("1000000000"))]
    pub amount_out: String,
    #[schema(examples("8iJxVDtFxnWpdCvdrgNDSXigxHo9vLf7KCS1pNKrs5Nh"))]
    pub maker: String,
    /// Total fees in lamports to be paid when building the transaction
    /// MMs to return us the fees they want to use either the suggested_fees in the quote request or a custom amount
    #[schema(examples("10000"))]
    pub prioritization_fee_to_use: Option<u64>,
    /// Taker is optional here as there are times we want to just get a quote without user signing in
    #[schema(examples("5v2Vd71VoJ1wZhz1PkhTY48mrJwS6wF4LfvDbYPnJ3bc"))]
    pub taker: Option<String>,
}

// #[derive(Clone, Debug, Deserialize, Serialize)]
// pub enum RequestStatus {
//     Accepted,
//     Rejected,
// }

/// Response to a swap request to the Market Maker
#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    #[schema(examples("59db3e19-c7b0-4753-a8aa-206701004498"))]
    pub quote_id: String,
    #[schema(examples("accepted", "rejected"))]
    pub state: SwapState,
    #[schema(examples("5K6CqVweTk4t9K6Xfa1gw7D9rS4GeAa8Z67e2q8Mi7f8QwexqTmtLnZgNeBe93PaRtt8beijqV9t7rp7C7yGfzkXGy2yFbF"), deprecated)]
    pub tx_signature: Option<String>,
    /// Optional message to provide more context when the swap is rejected
    pub rejection_reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    #[schema(examples("webhook api error message or code"))]
    pub message: String,
}

impl From<String> for ErrorResponse {
    fn from(message: String) -> Self {
        ErrorResponse { message }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenAddresses {
    #[schema(examples("JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add: Option<Vec<String>>,
    #[schema(examples("DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remove: Option<Vec<String>>,
}
