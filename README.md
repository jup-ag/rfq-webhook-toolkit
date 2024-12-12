# RFQ Webhook Toolkit

:exclamation: NOTE: This section is still heavily subjected to changes, and we are open to suggestions or feedbacks on ways to improve and streamline the integration.

## Market Maker Integration Guidelines


## Integration specifications

To facilitate the integration into Jupiter's RFQ module, you will need to provide a webhook for us to register the quotation and swap endpoints with the corresponding request and response format.

### Example URL that we will register into our api

```
https://your-api-endpoint.com/jupiter/rfq
```

Endpoints we will call after registration

```
POST https://your-api-endpoint.com/jupiter/rfq/quote
POST https://your-api-endpoint.com/jupiter/rfq/swap
```

#### API Key

If you require an API key to access your endpoints, please provide it to us during the registration process. The API Key will be passed to the webhook as a header `X-API-KEY`.

### POST /quote Request Response Format

We will send a POST request to the /quote endpoint with the following payload:

```http
POST /quote
Content-Type: application/json

{
   "requestId": "string", // UUIDv4
   "quoteId": "string", // UUIDv4
   "tokenIn": "string", // PublicKey
   "amount": "string",
   "tokenOut": "string", // PublicKey
   "quoteType": "string", // Literal: 'ExactIn' or 'ExactOut'
   "protocol": "string", // Literal: Protocol enum value
   "taker": "string | null", // Optional PublicKey
   "suggestedPrioritizationFees": "number | null" // Optional u64
}
```

And will expect a response within **500ms** with the following payload:

```http
200 OK
Content-Type: application/json

{
    "requestId": "string", // UUIDv4
    "quoteId": "string", // UUIDv4
    "tokenIn": "string", // PublicKey
    "tokenOut": "string", // PublicKey
    "quoteType": "string", // Literal: 'ExactIn' or 'ExactOut'
    "protocol": "string", // Literal: Protocol enum value
    "amountOut": "string",
    "amountIn": "string",
    "maker": "string",
    "prioritizationFeeToUse": "number | null",
    "taker": "string | null" // Optional PublicKey
}
```

### POST /swap Request Response Format

We will send a POST request to the /swap endpoint with the following payload:

```http
POST /quote
Content-Type: application/json

{
   "requestId": "string", // UUIDv4
   "quoteId": "string", // UUIDv4
   "transaction": "string" // Base64 encoded versioned transaction not base58
}
```

And will expect a response within **500ms** with the following payload:

```http
200 OK
Content-Type: application/json

{
    "quoteId": "string", // UUIDv4
    "state": "string", // "accepted",
    "txSignature": "string | null" // null if state != "accepted"
}
```

For any reasons that the MM have to bail the quotationa, you should reply the swap with

```http
200 OK
Content-Type: application/json

{
    "quoteId": "string", // UUIDv4
    "state": "string", // "rejected"
    "rejectionReason": "string | null", // This field is optional.
}
```

## Webhook Error Responses

Market Makers should return appropriate HTTP status codes along with error messages. The following status codes are supported:

### Severe errors

- `400 Bad Request`: When the request parameters sent by Jupiter are invalid or malformed
- `401 Unauthorized`: When authentication fails, e.g x-api-key is not provided or invalid

### Warnings

- `404 Not Found`: When the requested resource doesn't exist, as in case of a token not being supported

### Service errors

This is when the market maker is unable to fulfill the request due to internal issues. Jupiter may stop sending requests to the market maker if this happens too frequently.

- `500 Internal Server Error`: When an unexpected error occurs on the market maker's side
- `503 Service Unavailable`: When the market maker service is temporarily unavailable

Response format for errors should be:

```json
{
  "message": "string" // A descriptive error message
}
```

Example error response:

```http
403 Forbidden
Content-Type: application/json

{
    "message": "Quote size exceeds maximum allowed amount"
}
```

## Expiry information

We enforce a fixed expiry timing flow for all quotes and transactions:

1. When creating a quote, we set transaction expiry to **55 seconds** from creation time
2. On the frontend:
   - If remaining time before expiry is less than **40 seconds** when user needs to sign, we will automatically requote
   - The frontend will also do a requote every 5s
3. On the backend:
   - If remaining time before expiry is less than **25 seconds** when our /swap endpoint receives the request, we will reject the swap before forwarding to market makers

This fixed expiry flow simplifies the integration by:

- Removing the need for market makers to specify custom expiry times in quote requests
- Providing consistent behavior across all quotes and transactions
- Allowing for clear timeout boundaries at different stages of the flow

Note: These expiry thresholds may be adjusted based on performance and feedback.

## Future considerations/plans


#### Fulfillment Requirements

Market makers are expected to comply with 90% of the quotation swap requests provided before getting penalized.

#### Transaction Crafting

Current implementation enforces that Jupiter RFQ API will be the one crafting the instructions and transactions, however in the future we are working to improve on the flow to allow market makers to have the flexibility to craft their own transactions with a set of whitelisted instructions.

#### Transaction sending

Some market makers may not wish to be the ones handling the sending of transactions on chain. We may look into helping market makers land their transactions on chain in the future.


