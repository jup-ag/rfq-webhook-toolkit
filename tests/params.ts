
export const USDC = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';
export const WSOL = 'So11111111111111111111111111111111111111112';

// Base API URL, load from environment variable or use default
export const QUOTE_SERVICE_URL = process.env.QUOTE_SERVICE_URL || 'https://preprod.ultra-api.jup.ag';
export const WEBHOOK_ID = process.env.WEBHOOK_ID || false; // webhook id
export const TAKER_KEYPAIR = process.env.TAKER_KEYPAIR || "keypair.json"; // taker private key
export const AMOUNT = process.env.AMOUNT || 1_000_000;
export const MINT_A = process.env.MINT_A || WSOL;
export const MINT_B = process.env.MINT_B || USDC;
export const FEE_BPS = process.env.FEE_BPS || 2;

// Routers excluded from quote requests so the webhook under test is the only
// liquidity source (issue #49). When Jupiter adds a new router, update this
// single line; keep in sync with the `router` enum in the /order docs:
// https://developers.jup.ag/docs/swap/order-and-execute
export const EXCLUDED_ROUTERS = 'metis,hashflow,dflow,pyth,okx';

