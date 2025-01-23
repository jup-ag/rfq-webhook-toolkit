import { describe, it, expect } from 'vitest';
import axios from 'axios';
import { assert } from 'chai';
const BN = require('bn.js');
const solanaWeb3 = require('@solana/web3.js');
const fs = require('fs');
import * as params from '../../params';


// Start mock server before tests and close it after
describe('Webhook EDGE API Swap', {
  timeout: 10_000,
},() => {
  it('should execute a successful swap', async () => {

    assert(params.WEBHOOK_ID, 'WEBHOOK_ID is not set');
    assert(params.TAKER_KEYPAIR, 'TAKER_KEYPAIR is not set');

    // Read the keypair
    const keypair = solanaWeb3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(params.TAKER_KEYPAIR, 'utf8'))));
    const taker = keypair.publicKey.toString();
    console.log('taker address: ', taker);

    const quoteURL = `${params.QUOTE_SERVICE_URL}/quote`;
    console.log('request url: ', quoteURL);

    const quoteParams = {
      swapMode: params.SWAP_MODE, // TODO:  check this field
      taker: taker,
      inputMint: params.INPUT_MINT,
      outputMint: params.OUTPUT_MINT,
      amount: `${params.AMOUNT}`,
      swapType: 'rfq',
      webhookId: params.WEBHOOK_ID,
    }

    try {
      // Step 1: Fetch the quote
      const quoteResponse = await axios.get(quoteURL, { params: quoteParams });
      console.log("Quote response --> ", quoteResponse.data);

      // Assertions for the quote response
      expect(quoteResponse.status).toBe(200);
      expect(quoteResponse.data).toHaveProperty('quoteId');
      expect(quoteResponse.data).toHaveProperty('requestId');
      expect(quoteResponse.data).toHaveProperty('expireAt');
      expect(quoteResponse.data).toHaveProperty('orderInfo');
      expect(quoteResponse.data).toHaveProperty('maker');
      expect(quoteResponse.data.orderInfo.input.startAmount).toBe(`${params.AMOUNT}`);
      expect(quoteResponse.data.orderInfo.input.token).toBe(params.INPUT_MINT);
      expect(new BN(quoteResponse.data.orderInfo.output.startAmount).gt(new BN(0))).toBe(true);
      expect(quoteResponse.data.orderInfo.output.token).toBe(params.OUTPUT_MINT);

      // Step 2: Transaction signing
      const base64Transaction = quoteResponse.data.transaction;
      expect(base64Transaction).toBeDefined();

      const transactionBytes = Buffer.from(base64Transaction, 'base64');
      const transaction = solanaWeb3.VersionedTransaction.deserialize(transactionBytes);
      transaction.sign([keypair]);
      const signedTransactionBase64 = Buffer.from(transaction.serialize()).toString('base64');

      // Step 3: Send the swap transaction
      const swapURL = `${params.QUOTE_SERVICE_URL}/swap`;
      const swapPayload = {
        quoteId: quoteResponse.data.quoteId,
        requestId: quoteResponse.data.requestId,
        transaction: signedTransactionBase64,
      };
      const swapParams = { swapType: 'rfq' };
      console.log("Swap payload --> ", swapPayload);

      const swapResponse = await axios.post(swapURL, swapPayload, { params: swapParams });
      console.log("Swap response --> ", swapResponse.data);

      // Assertions for the swap response
      expect(swapResponse.status).toBe(200);
      expect(swapResponse.data.quoteId).toBe(swapPayload.quoteId);
      expect(swapResponse.data.state).toBe("accepted");

    } catch (error) {
      if (error.response) {
        console.error("Error from request: ", error.config.url);
        console.log("Error response data --> ", error.response.data);
        assert.fail(
          `Request to ${error.config.url} failed with status ${error.response.status}: ${error.response.data.error}`
        );
      } else if (error.request) {
        console.error("Error from request: ", error.config.url);
        console.log("Error request --> ", error.request);
        assert.fail(`Request to ${error.config.url} failed: no response from server`);
      } else {
        console.log("Error --> ", error);
        assert.fail("Unknown error occurred");
      }
    }
  });
});
