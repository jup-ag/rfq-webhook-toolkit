import { describe, it, expect } from 'vitest';
import axios from 'axios';
import { assert } from 'chai';
const BN = require('bn.js');
const solanaWeb3 = require('@solana/web3.js');
const fs = require('fs');



// Base API URL, load from environment variable or use default
const QUOTE_SERVICE_URL = process.env.QUOTE_SERVICE_URL || 'https://quote-proxy-edge.raccoons.dev';
const WEBHOOK_ID = process.env.WEBHOOK_ID || false; // webhook id
const TAKER_KEYPAIR = process.env.TAKER_KEYPAIR || "keypair.json"; // taker private key
const AMOUNT = process.env.AMOUNT || 1_000_000; // swap 1 USDC for SOL
const INPUT_MINT = process.env.INPUT_MINT || 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v'; // USDC
const OUTPUT_MINT = process.env.OUTPUT_MINT || 'So11111111111111111111111111111111111111112'; // wSOL (will be converted to native SOL)

// optional fields
const SWAP_MODE = process.env.SWAP_MODE || 'exactIn'; // 'exactIn' or 'exactOut'

// Helper function to validate Solana addresses
function isValidSolanaAddress(address) {
  try {
    // Attempt to create a PublicKey, which validates the address format
    new solanaWeb3.PublicKey(address);
    return true;
  } catch (e) {
    return false;
  }
}

// Start mock server before tests and close it after
describe('Webhook EDGE API Quote', () => {
  it('should return a successful quote response', async () => {

    assert(WEBHOOK_ID, 'WEBHOOK_ID is not set');
    assert(TAKER_KEYPAIR, 'TAKER_KEYPAIR is not set');

    // Read the keypair
    const keypair = solanaWeb3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(TAKER_KEYPAIR, 'utf8'))));
    const taker = keypair.publicKey.toString();
    console.log('taker address: ', taker);

    const url = `${QUOTE_SERVICE_URL}/quote`;
    console.log('request url: ', url);

    const payload = {
      swapMode: SWAP_MODE, // TODO: change to swapType
      taker: taker,
      inputMint: INPUT_MINT,
      outputMint: OUTPUT_MINT,
      amount: `${AMOUNT}`,
      swapType: 'rfq',
      webhookId: WEBHOOK_ID,
    }

    const response = await axios.get(url, { params: payload });

    console.log("response --> ", response.data);

    expect(response.status).toBe(200);
    expect(response.data).toHaveProperty('quoteId');
    expect(response.data).toHaveProperty('requestId');
    expect(response.data).toHaveProperty('expireAt');
    expect(response.data).toHaveProperty('orderInfo');
    expect(response.data).toHaveProperty('maker'); // the maker should be the MM address
    expect(response.data).toHaveProperty('orderInfo');
    expect(response.data.orderInfo.input.startAmount).toBe(`${AMOUNT}`);
    expect(response.data.orderInfo.input.token).toBe(INPUT_MINT);
    expect(new BN(response.data.orderInfo.output.startAmount).gt(new BN(0))).toBe(true);
    expect(response.data.orderInfo.output.token).toBe(OUTPUT_MINT);
  });
});
