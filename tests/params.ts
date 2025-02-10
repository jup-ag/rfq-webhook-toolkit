
export const USDC = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';
export const WSOL = 'So11111111111111111111111111111111111111112';

// Base API URL, load from environment variable or use default
export const QUOTE_SERVICE_URL = process.env.QUOTE_SERVICE_URL || 'https://quote-proxy-edge.raccoons.dev';
export const WEBHOOK_ID = process.env.WEBHOOK_ID || false; // webhook id
export const TAKER_KEYPAIR = process.env.TAKER_KEYPAIR || "keypair.json"; // taker private key
export const AMOUNT = process.env.AMOUNT || 1_000_000;
export const MINT_A = process.env.MINT_A || WSOL;
export const MINT_B = process.env.MINT_B || USDC;
export const FEE_BPS = process.env.FEE_BPS || 2;
