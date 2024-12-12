use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::enums::{Protocol, QuoteType};

/// Request to get a quote from the Market Maker
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    #[schema(examples("629bddf3-0038-43a6-8956-f5433d6b1191"))]
    pub request_id: String,
    #[schema(examples("59db3e19-c7b0-4753-a8aa-206701004498"))]
    pub quote_id: String,
    #[schema(examples("So11111111111111111111111111111111111111112"))]
    pub token_in: String,
    #[schema(examples("250000000"))]
    pub amount_in: String, //
    #[schema(examples("250000000"))]
    pub amount: String, // TODO: deprecate this in favor of amount_in
    #[schema(examples("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"))]
    pub token_out: String,
    #[schema(examples("ExactIn"), read_only = true)]
    pub quote_type: QuoteType,
    #[schema(examples("v1"), read_only = true)]
    pub protocol: Protocol,
    /// Taker is optional here as there are times we want to just get a quote without user signing in
    /// When user signs in, we should try to requote again so the new quote request will have a taker
    #[schema(examples("5v2Vd71VoJ1wZhz1PkhTY48mrJwS6wF4LfvDbYPnJ3bc"))]
    pub taker: Option<String>,
    /// If no taker is provided, the there will be no suggested fee
    /// This is the suggested total fees in lamports to be paid when building the transaction
    /// MMs will have the option to ignore our suggested fee and provide their own when responding to this quote
    #[schema(examples("10000"))]
    pub suggested_prioritization_fees: Option<u64>,
}

/// Order to be fulfilled by the Market Maker
#[derive(Clone, Deserialize, Serialize, Debug, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    #[schema(examples("629bddf3-0038-43a6-8956-f5433d6b1191"))]
    pub request_id: String,
    #[schema(examples("59db3e19-c7b0-4753-a8aa-206701004498"))]
    pub quote_id: String,
    /// Base64 encoded versioned transaction
    #[schema(examples("AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAIABgxSnEehNb4kLTrfnzoVcTu/GPLBwP0kKZRTJowyLvHxxSdkl7oLuGWRcrcu3Yxm4Y9WF2TZyGphCjp+D3nAuvnbWolfQZ0Kl+9/uOLLKVXoXu/o/NQI5LY9pgx8ibLVfztqKpSdlIRAyuBnIsFa1A93abdI4AmIcbFLGFGatrhAXnMzpil7FnByGEuo10mEgCYqn/QfD1DTR6idALqAu9Bhh6NTL/nu9FDLsM2mMKzzPPKY2nBeuUHR7ibnmbqVw/MAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMGRm/lIRcy/+ytunLDm+e8jOW7xfcSayxDmzpAAAAABpuIV/6rgYT7aH9jRhjANdrEOdwa6ztVmKDwAAAAAAEG3fbh12Whk9nL4UbO63msHLSF7V9bN5E6jPWFfv8AqUpYSftyo7vpH9xbDmpX9jxaHLRbIGem7Qys02OVyKECxvp6877brTo9ZfNqq8l0MbG75MLS9uDkfKYCA0UvXWE9f/tHi80zsUphEh9edGz8h7JFCM8ITLeBpkq6CPTyfQMHAAkDmLEBAAAAAAAHAAUCwFwVAAoMAQACAwoFCwkICQYEIKhgt6NcCiigAMqaOwAAAAAm/0kBAQAAAEAvW2cAAAAAAA=="))]
    pub transaction: String,
}
