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
    pub amount: String,
    #[schema(examples("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"))]
    pub token_out: String,
    #[schema(examples("exactIn"), read_only = true)]
    pub quote_type: QuoteType,
    #[schema(examples("v1"), read_only = true)]
    pub protocol: Protocol,
    /// Taker is optional here as there are times we want to just get a quote without user signing in
    /// When user signs in, we should try to requote again so the new quote request will have a taker
    #[schema(examples("5v2Vd71VoJ1wZhz1PkhTY48mrJwS6wF4LfvDbYPnJ3bc"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taker: Option<String>,
    /// If no taker is provided, the there will be no suggested fee
    /// This is the suggested compute unit price in micro lamports to be set when building the transaction
    /// MMs will have the option to ignore our suggested fee and provide their own when responding to this quote
    #[schema(examples("10000"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_prioritization_fees: Option<u64>,
    /// Fee in basis points to be charged by the Market Maker
    #[schema(examples("1", "20"), maximum = 10_000)]
    pub fee_bps: u16,
    /// Flag to indicate if token is wSOL
    #[schema(examples("true", "false"))]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_token_in_wsol: Option<bool>,
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
    #[schema(examples("AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAz9drCWYYQ68kASuBn9OHQMhApFELvj44L1s76RFusqcFQ3u65aig44TQ3Fmb9CadUg6y5zJuBNnD1IxqvKXIPmA4AmO5Dcos4MycwafOIB13mDRFQ1GIRqKG3olkhi48jyGiqvTscHPp0TmqflJdR4gzVibQqwIj1iO1jXHw5Mt99q5m2Edp3glkLYOc/yT1HqD+ndBXyPYu16F84mC8rspYEafRZphIlog6Q2qO4TFgN8ICPW2yl1kkJ2UutYEAxh1w4ztXWtKZr0O736NcYMPOKkRjP8CiDXheWMdaprkzkaA5jAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAvEC3LPJppXp/7XKg0lfT6E87hQi8th5zPmi1Q6AAAAAiDuU6+gSRr+Hx/3xlmJE16rC5kSGnxcWkohAAAAAAAg5izz2u1OeCnop+hOiUf4tBZ6V2mJYyUZ0OXgw2U/A4C33oSysI64Na/dJmwBs/WKYt6Nnkl1JWNR65pjlN4nAKWsCeLSm9f59f75OR3BSLGqzUjgzq3orAORpoFbS1sy0skiTfwZdbBKqGpHSo0ZZfwJkHDO1fB4frglAsUPj0YoKeHP5JwEP4awSwmz4vanicFfQDIeA6ZM8UhZEZe0FLBZAAyNq5O0AAAAAAA2AAagQ9iAAAlYAMq28zUDAlb3EiKAFj7JAPtooroIJbdZdUpVUmdUfgBMAAAAAAH4YYwAAAAAAC9MP4tAAAAAAA=="))]
    pub transaction: String,
}
