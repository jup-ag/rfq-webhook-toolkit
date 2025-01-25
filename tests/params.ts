


// Base API URL, load from environment variable or use default
export const QUOTE_SERVICE_URL = process.env.QUOTE_SERVICE_URL || 'https://quote-proxy-edge.raccoons.dev';
export const WEBHOOK_ID = process.env.WEBHOOK_ID || false; // webhook id
export const TAKER_KEYPAIR = process.env.TAKER_KEYPAIR || "keypair.json"; // taker private key
export const AMOUNT = process.env.AMOUNT || 1_000_000; // swap 1 USDC for SOL
export const INPUT_MINT = process.env.INPUT_MINT || 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v'; // USDC
export const OUTPUT_MINT = process.env.OUTPUT_MINT || 'So11111111111111111111111111111111111111112'; // wSOL (will be converted to native SOL)
export const FEE_BPS = process.env.FEE_BPS || 0;

// optional fields
export const SWAP_MODE = process.env.SWAP_MODE || 'ExactIn'; // 'ExactIn' or 'ExactOut'