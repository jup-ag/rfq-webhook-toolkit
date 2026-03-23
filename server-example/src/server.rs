/// Copyright 2024 Raccoons
///
/// This file is part of Raccoons, which is released under the MIT license.
///
/// This file is the main entry point for the REST server. It is responsible for
/// parsing command line arguments and starting the server.
///
///
use anyhow::Result;
use order_engine_sdk::transaction::{
    deserialize_transaction_base64_into_transaction_details, TransactionDetails,
};
use solana_rpc_client::rpc_client::SerializableTransaction;
use solana_sdk::{
    signature::Keypair,
    signer::{EncodableKey, Signer},
};
use thiserror::Error;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use utoipauto::utoipauto;

use std::{collections::HashMap, sync::Arc, time::Duration};

use axum::{
    extract::{rejection::JsonRejection, Query, State},
    http::{HeaderMap, StatusCode},
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

use webhook_api::{
    enums::{QuoteType, RejectionReason, SwapState},
    requests::*,
    responses::*,
};

use crate::config::Config;

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
                tracing::error!("JsonExtractorRejection: {:?}", json_rejection);
                (json_rejection.status(), json_rejection.body_text())
            }
            Self::NotFound() => (StatusCode::NOT_FOUND, self.to_string()),
            Self::BadRequest(error) => {
                tracing::error!("BadRequest: {:?}", error);
                (StatusCode::BAD_REQUEST, error)
            }
            Self::GenericError(error) => {
                tracing::error!("GenericError: {:?}", error);
                (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
            }
        };

        (status, Json(ErrorResponse::from(message))).into_response()
    }
}

// **************************************
// OpenAPI docs
// **************************************

/// OpenAPI spec for the API
/// This is used to generate the OpenAPI spec for the API
#[utoipauto(paths = "server-example/src")]
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
    State(state): State<Arc<AppState>>,
    Query(_queries): Query<HashMap<String, String>>,
    headers: HeaderMap,
    WithRejection(Json(quote_request), _): WithRejection<Json<QuoteRequest>, ApiError>,
) -> Result<Json<QuoteResponse>, ApiError> {
    tracing::info!(
        "Received quote request: {:?}, headers: {:?}",
        quote_request,
        headers
    );

    let is_input_mint_supported = state
        .config
        .supported_tokens
        .contains(&quote_request.token_in);

    let is_output_mint_supported = state
        .config
        .supported_tokens
        .contains(&quote_request.token_out);

    // if the  the token pair is not supported, return 404
    if !is_input_mint_supported || !is_output_mint_supported {
        return Err(ApiError::NotFound());
    }

    // The normal flow of a quote request would be:
    // Step 1: Parse the request
    // Step 2: Compute the quote
    // Step 3: Build the quote response

    let maker_pubkey = state.keypair.pubkey().to_string();

    let example_quoted_amount = "123123123".to_string();

    // different logic between ExactIn and ExactOut

    let quote = match quote_request.quote_type {
        QuoteType::ExactIn => {
            // compute the amount out based on the amount in
            QuoteResponse {
                request_id: quote_request.request_id,
                quote_id: quote_request.quote_id,
                taker: quote_request.taker,
                token_in: quote_request.token_in,
                amount_in: quote_request.amount.clone(),
                token_out: quote_request.token_out,
                quote_type: quote_request.quote_type,
                protocol: quote_request.protocol,
                amount_out: example_quoted_amount,
                maker: maker_pubkey,
                prioritization_fee_to_use: quote_request.suggested_prioritization_fees,
                receiver: quote_request.receiver.clone(),
            }
        }
        QuoteType::ExactOut => {
            // compute the amount in based on the amount out
            QuoteResponse {
                request_id: quote_request.request_id,
                quote_id: quote_request.quote_id,
                taker: quote_request.taker,
                token_in: quote_request.token_in,
                amount_in: example_quoted_amount,
                token_out: quote_request.token_out,
                quote_type: quote_request.quote_type,
                protocol: quote_request.protocol,
                amount_out: quote_request.amount.clone(),
                maker: maker_pubkey,
                prioritization_fee_to_use: quote_request.suggested_prioritization_fees,
                receiver: quote_request.receiver.clone(),
            }
        }
    };

    // Build jupiter quote request
    Ok(Json(quote))
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
    State(state): State<Arc<AppState>>,
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
    const SIMULATE_INSUFFICIENT_BALANCE: &str = "00000000-0000-0000-0000-000000000003";
    const SIMULATE_SIGNATURE_VERIFICATION_FAILED: &str = "00000000-0000-0000-0000-000000000004";

    match quote_request.request_id.as_str() {
        SIMULATE_REJECTION => Ok(Json(SwapResponse {
            tx_signature: None,
            quote_id: quote_request.quote_id.clone(),
            state: SwapState::Rejected,
            rejection_reason: Some("<rejection reason>".to_string()),
        })),
        SIMULATE_INSUFFICIENT_BALANCE => Ok(Json(SwapResponse {
            tx_signature: None,
            quote_id: quote_request.quote_id.clone(),
            state: SwapState::RejectedWithReason(RejectionReason::InsufficientBalance),
            rejection_reason: None,
        })),
        SIMULATE_SIGNATURE_VERIFICATION_FAILED => Ok(Json(SwapResponse {
            tx_signature: None,
            quote_id: quote_request.quote_id.clone(),
            state: SwapState::RejectedWithReason(RejectionReason::SignatureVerificationFailed),
            rejection_reason: None,
        })),
        SIMULATE_MALFORMED => Err(ApiError::BadRequest("Malformed request".to_string())),
        _ => {
            // ========================================
            // extract the message
            // ========================================
            let TransactionDetails {
                mut versioned_transaction,
                sanitized_message: _,
            } = deserialize_transaction_base64_into_transaction_details(
                &quote_request.transaction,
            )?;

            // ========================================
            // validate the message
            // ========================================
            // add the validation logic here

            // ========================================
            // add the maker signature to the transaction
            // ========================================
            match versioned_transaction.signatures.get_mut(0) {
                Some(signature_slot) => {
                    let signature = state
                        .keypair
                        .sign_message(&versioned_transaction.message.serialize());
                    *signature_slot = signature;
                }
                None => {
                    return Err(ApiError::BadRequest(
                        "Partial sign signature to replace not found".to_string(),
                    ));
                }
            }
            let signature = versioned_transaction.get_signature().to_string();

            // ========================================
            // broadcast the transaction
            // ========================================

            let _rpc_client_url = state.config.rpc_url.clone();
            /*
            tokio::spawn(async move {
                let client = nonblocking::rpc_client::RpcClient::new(rpc_client_url);
                // We're using send_and_confirm_transaction to keep sending until the outcome is known on-chain, or the blockhash expires
                if let Err(error) = client
                    .send_and_confirm_transaction(&versioned_transaction)
                    .await
                {
                    tracing::error!("Failed to send and confirm transaction: {error:?}");
                };
            });
             */

            // return the response
            Ok(Json(SwapResponse {
                tx_signature: Some(signature.to_string()),
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
async fn example_tokens_list(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<String>>, ApiError> {
    Ok(Json(state.config.supported_tokens.clone()))
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

struct AppState {
    config: Config,
    keypair: Keypair,
}

fn app(state: Arc<AppState>) -> Router {
    let router = Router::new()
        .route("/quote", post(example_quote))
        .route("/swap", post(example_swap))
        .route("/tokens", get(example_tokens_list))
        // not part of RFQ spec, but useful for debugging
        .route("/health", get(get_health))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-doc/openapi.json", ApiDoc::openapi()))
        .fallback(not_found_handler)
        .with_state(state);

    router
        .layer(CorsLayer::permissive().max_age(MAX_AGE))
        //.layer(TimeoutLayer::new(Duration::from_secs(10)))
        .layer(
            TraceLayer::new_for_http()
                .on_response(DefaultOnResponse::new().level(TraceLevel::INFO))
                .on_failure(DefaultOnFailure::new().level(TraceLevel::ERROR)),
        )
}

pub async fn serve(config: Config) {
    // generate a keypair if not provided
    let keypair = match &config.maker_keypair {
        Some(private_key_file) => {
            tracing::info!("loading keypair from file: {}", private_key_file);
            Keypair::read_from_file(private_key_file).expect("Invalid keypair file")
        }
        None => {
            tracing::info!("Loaded keypair from file");
            Keypair::new()
        }
    };

    tracing::info!("maker pubkey: {}", keypair.pubkey());

    // create the shared state
    let app_state = Arc::new(AppState {
        config: config.clone(),
        keypair,
    });

    // build the axum router
    let app = app(app_state);
    // start the server
    let listener = tokio::net::TcpListener::bind(config.listen_addr.clone())
        .await
        .unwrap();
    tracing::info!("Starting server at {:?}", config.listen_addr);
    axum::serve(listener, app).await.unwrap();
}
