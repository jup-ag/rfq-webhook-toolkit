/// Copyright 2024 Raccoons
///
/// This file is part of Raccoons, which is released under the MIT license.
///
/// This file is the main entry point for the REST server. It is responsible for
/// parsing command line arguments and starting the server.
///
///
use anyhow::Result;
use once_cell::sync::Lazy;
use thiserror::Error;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use utoipauto::utoipauto;

use std::{collections::HashMap, sync::Arc, time::Duration, vec};

use axum::{
    extract::{rejection::JsonRejection, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::WithRejection;

use tower_http::{
    cors::CorsLayer,
    trace::{DefaultOnFailure, DefaultOnResponse, TraceLayer},
};

use tracing::Level as TraceLevel;

use webhook_api::{enums::SwapState, requests::*, responses::*};

use crate::config::Config;

static SUPPORTED_TOKENS: Lazy<Vec<String>> = Lazy::new(|| {
    vec![
        "So11111111111111111111111111111111111111112".to_string(),
        "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
        "JUPyiwrYJFskUPiHa7hkeR8VUtAeFoSYbKedZNsDvCN".to_string(),
    ]
});

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("API method not found")]
    NotFound(),
    #[error("{0}")]
    BadRequest(String),
    // handle errors for incoming invalid json
    #[error(transparent)]
    JsonExtractorRejection(#[from] JsonRejection),
    // Catch all, generic error from anyhow
    #[error(transparent)]
    GenericError(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            Self::JsonExtractorRejection(json_rejection) => {
                (json_rejection.status(), json_rejection.body_text())
            }
            Self::NotFound() => (StatusCode::NOT_FOUND, self.to_string()),
            Self::BadRequest(error) => (StatusCode::BAD_REQUEST, error),
            Self::GenericError(error) => (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
        };

        (status, Json(ErrorResponse::from(message))).into_response()
    }
}

// **************************************
// OpenAPI docs
// **************************************

/// OpenAPI spec for the API
/// This is used to generate the OpenAPI spec for the API
#[utoipauto(paths = "./server-example/src")]
#[derive(OpenApi)]
#[openapi(
    tags(
        (name = "RFQ Webhook API", description = "Webhook API for RFQ providers"),
    )
)]
pub struct ApiDoc;

// **************************************
// Handlers
// **************************************

/// Example quote handler
///
/// This is an example quote handler that returns a hardcoded quote response

// add utoipa annotations to the handler
#[utoipa::path(post, path = "/quote",
params(
    ("X-API-KEY" = Option<String>, Header, description = "Optional API Key (if required by the webhook)"),
),
responses(
    (status = 200, body= QuoteResponse),
    (status = 400, body= ErrorResponse),
    (status = 401, body= ErrorResponse),
    (status = 404, body= ErrorResponse),
    (status = 500, body= ErrorResponse),
    (status = 503, body= ErrorResponse),
))]
async fn example_quote(
    State(config): State<Arc<Config>>,
    Query(_queries): Query<HashMap<String, String>>,
    WithRejection(Json(quote_request), _): WithRejection<Json<QuoteRequest>, ApiError>,
) -> Result<Json<QuoteResponse>, ApiError> {
    tracing::info!("Received quote request: {:?}", quote_request);

    // if the  the token pair is not supported, return 404
    if !SUPPORTED_TOKENS.contains(&quote_request.token_in)
        || !SUPPORTED_TOKENS.contains(&quote_request.token_out)
    {
        return Err(ApiError::NotFound());
    }

    // The normal flow of a quote request would be:
    // Step 1: Parse the request
    // Step 2: Compute the quote
    // Step 3: Build the quote response

    let quote_res = QuoteResponse {
        request_id: quote_request.request_id,
        quote_id: quote_request.quote_id,
        taker: quote_request.taker,
        token_in: quote_request.token_in,
        amount_in: quote_request.amount.clone(),
        token_out: quote_request.token_out,
        quote_type: quote_request.quote_type,
        protocol: quote_request.protocol,
        amount_out: "100000000".to_string(), // hardcoded amount out
        maker: config.maker_address.to_string(),
        prioritization_fee_to_use: quote_request.suggested_prioritization_fees,
    };

    // Build jupiter quote request
    Ok(Json(quote_res))
}

/// Example swap handler
///
/// This is an example swap handler that showcase how to execute a swap from the MM side

#[utoipa::path(post, path = "/swap",
params(
    ("X-API-KEY" = Option<String>, Header, description = "Optional API Key (if required by the webhook)"),
),
responses(
    (status = 200, body= SwapResponse),
    (status = 400, body= ErrorResponse),
    (status = 401, body= ErrorResponse),
    (status = 500, body= ErrorResponse),
    (status = 503, body= ErrorResponse),
))]
async fn example_swap(
    Query(_queries): Query<HashMap<String, String>>,
    WithRejection(Json(quote_request), _): WithRejection<Json<SwapRequest>, ApiError>,
) -> Result<Json<SwapResponse>, ApiError> {
    // Step 1: Parse the request
    // Step 2: Sign the transaction
    // Step 3: Send the transaction
    // Step 4: Build the swap response with the tx signature

    // For testing purposes on this implementation  we leverage ad-hoc request_id to trigger errors

    const SIMULATE_REJECTION: &str = "00000000-0000-0000-0000-000000000001";
    const SIMULATE_MALFORMED: &str = "00000000-0000-0000-0000-000000000002";

    match quote_request.request_id.as_str() {
        SIMULATE_REJECTION =>
            Ok(Json(SwapResponse {
                tx_signature: None,
                quote_id: quote_request.quote_id.clone(),
                state: SwapState::Rejected,
                rejection_reason: Some("<rejection reason>".to_string()),
            }))
        ,
        SIMULATE_MALFORMED =>
            Err(ApiError::BadRequest("Malformed request".to_string())),
        _ => {
            Ok(Json(SwapResponse {
                tx_signature: Some("3HMNN9enUZnjj2eV3vBB8j3RWtmq1iSpXniRd4Ly41vjx7PoAUjAcRz1Cz2FX8YZBkPnj2Lzew2YFPcSkkbp85Xj".to_string()),
                quote_id: quote_request.quote_id.clone(),
                state: SwapState::Accepted,
                rejection_reason: None,
            }))
        }
    }
}

#[utoipa::path(get, path = "/tokens",
params(
    ("X-API-KEY" = Option<String>, Header, description = "Optional API Key (if required by the webhook)"),
),
responses(
    (status = 200, body= Vec<String>),
    (status = 400, body= ErrorResponse),
))]
pub async fn example_tokens_list() -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(SUPPORTED_TOKENS.clone()))
}

async fn get_health() -> Result<(), ApiError> {
    Ok(())
}

async fn not_found_handler() -> ApiError {
    ApiError::NotFound()
}

// **************************************
// Axum router setup
// **************************************

const MAX_AGE: Duration = Duration::from_secs(86400);

pub fn app(config: Arc<Config>) -> Router {
    let router = Router::new()
        .route("/quote", post(example_quote))
        .route("/swap", post(example_swap))
        .route("/tokens", get(example_tokens_list))
        // not part of RFQ spec, but useful for debugging
        .route("/health", get(get_health))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-doc/openapi.json", ApiDoc::openapi()))
        .fallback(not_found_handler)
        .with_state(config);

    router
        .layer(CorsLayer::permissive().max_age(MAX_AGE))
        //.layer(TimeoutLayer::new(Duration::from_secs(10)))
        .layer(
            TraceLayer::new_for_http()
                .on_response(DefaultOnResponse::new().level(TraceLevel::INFO))
                .on_failure(DefaultOnFailure::new().level(TraceLevel::ERROR)),
        )
}

pub async fn serve(config: Arc<Config>) {
    // build the axum router
    let app = app(config.clone());
    // start the server
    let listener = tokio::net::TcpListener::bind(config.listen_addr.clone())
        .await
        .unwrap();
    tracing::info!("Starting server at {:?}", config.listen_addr);
    axum::serve(listener, app).await.unwrap();
}
