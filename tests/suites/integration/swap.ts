import axios from 'axios';
import { assert } from 'chai';
import { describe, expect, it } from 'vitest';
import * as params from '../../params';
import { loadKeypairFromFile } from '../../helpers';
import { KeyPairSigner, appendTransactionMessageInstruction, compileTransaction, createTransactionMessage, decompileTransactionMessage, getBase64Decoder, getBase64Encoder, getCompiledTransactionMessageDecoder, getTransactionDecoder, getTransactionEncoder, partiallySignTransaction, pipe, signTransaction } from '@solana/kit';
import { BN } from 'bn.js';
import { getAssertAccountInfoInstruction, accountInfoAssertion, IntegerOperator } from 'lighthouse-sdk';
import test from 'node:test';
describe('Webhook e2e API Swap', {
  timeout: 10_000,
  // skip: true
}, () => {
  it('should execute a successful swap (ExactIn)', async () => {

    assert(params.WEBHOOK_ID, 'WEBHOOK_ID is not set');
    assert(params.TAKER_KEYPAIR, 'TAKER_KEYPAIR is not set');

    console.log(`how many ${params.MINT_A} will you get for ${params.AMOUNT} of ${params.MINT_B}?`);

    // Read the keypair
    const keypair: KeyPairSigner = await loadKeypairFromFile(params.TAKER_KEYPAIR);
    const taker = keypair.address;
    console.log('taker address: ', taker);

    const quoteURL = `${params.QUOTE_SERVICE_URL}/order`;
    console.log('request url: ', quoteURL);

    const quoteParams = {
      inputMint: params.MINT_B,
      outputMint: params.MINT_A,
      amount: `${params.AMOUNT}`,
      mode: "manual",
      swapMode: "ExactIn",
      slippageBps: 50,
      broadcastFeeType: "maxCap",
      priorityFeeLamports: 1000000,
      useWsol: false,
      asLegacyTransaction: false,
      excludeDexes: "",
      excludeRouters: "metis%2Chashflow%2Cdflow",
      taker: taker,
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
      expect(quoteResponse.data).toHaveProperty('maker');
      expect(quoteResponse.data.inAmount).toBe(`${params.AMOUNT}`);
      expect(quoteResponse.data.inputMint).toBe(params.MINT_B);
      expect(new BN(quoteResponse.data.outAmount).gt(new BN(0))).toBe(true);
      expect(quoteResponse.data.outputMint).toBe(params.MINT_A);


      // Step 2: Transaction signing
      const base64Transaction = quoteResponse.data.transaction;
      expect(base64Transaction).toBeDefined();

      const transactionBytes = getBase64Encoder().encode(base64Transaction);
      const transaction = getTransactionDecoder().decode(transactionBytes);


      const signedTransaction = await pipe(
        transaction,
        (tx) => partiallySignTransaction([keypair.keyPair], tx),
      );

      const signedTransactionBytes = getTransactionEncoder().encode(signedTransaction);
      const signedTransactionBase64 = getBase64Decoder().decode(signedTransactionBytes);

      // Step 3: Send the swap transaction
      const swapURL = `${params.QUOTE_SERVICE_URL}/execute`;
      const swapPayload = {
        requestId: quoteResponse.data.requestId,
        signedTransaction: signedTransactionBase64,
      };
      const swapParams = { swapType: 'rfq' };
      console.log("Swap payload --> ", swapPayload);

      const swapResponse = await axios.post(swapURL, swapPayload, { params: swapParams });
      console.log("Swap response --> ", swapResponse.data);

      // Assertions for the swap response
      expect(swapResponse.status).toBe(200);
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


  it('should execute a successful swap (ExactOut)', async () => {

    assert(params.WEBHOOK_ID, 'WEBHOOK_ID is not set');
    assert(params.TAKER_KEYPAIR, 'TAKER_KEYPAIR is not set');

    console.log(`how many ${params.MINT_A} do you need to get ${params.AMOUNT} of ${params.MINT_B}?`);

    // Read the keypair
    const keypair: KeyPairSigner = await loadKeypairFromFile(params.TAKER_KEYPAIR);
    const taker = keypair.address;
    console.log('taker address: ', taker);

    const quoteURL = `${params.QUOTE_SERVICE_URL}/order`;
    console.log('request url: ', quoteURL);

    const quoteParams = {
      inputMint: params.MINT_A,
      outputMint: params.MINT_B,
      amount: `${params.AMOUNT}`,
      mode: "manual",
      swapMode: "ExactOut",
      slippageBps: 50,
      broadcastFeeType: "maxCap",
      priorityFeeLamports: 1000000,
      useWsol: false,
      asLegacyTransaction: false,
      excludeDexes: "",
      excludeRouters: "metis%2Chashflow%2Cdflow",
      taker: taker,
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
      expect(quoteResponse.data).toHaveProperty('maker');
      expect(quoteResponse.data.outAmount).toBe(`${params.AMOUNT}`);
      expect(quoteResponse.data.outputMint).toBe(params.MINT_B);
      expect(new BN(quoteResponse.data.inAmount).gt(new BN(0))).toBe(true);
      expect(quoteResponse.data.inputMint).toBe(params.MINT_A);

      // Step 2: Transaction signing
      const base64Transaction = quoteResponse.data.transaction;
      expect(base64Transaction).toBeDefined();

      const transactionBytes = getBase64Encoder().encode(base64Transaction);
      const transaction = getTransactionDecoder().decode(transactionBytes);

      const signedTransaction = await pipe(
        transaction,
        (tx) => partiallySignTransaction([keypair.keyPair], tx),
      );

      const signedTransactionBytes = getTransactionEncoder().encode(signedTransaction);
      const signedTransactionBase64 = getBase64Decoder().decode(signedTransactionBytes);

      // Step 3: Send the swap transaction
      const swapURL = `${params.QUOTE_SERVICE_URL}/execute`;
      const swapPayload = {
        requestId: quoteResponse.data.requestId,
        signedTransaction: signedTransactionBase64,
      };
      const swapParams = { swapType: 'rfq' };
      console.log("Swap payload --> ", swapPayload);

      const swapResponse = await axios.post(swapURL, swapPayload, { params: swapParams });
      console.log("Swap response --> ", swapResponse.data);

      // Assertions for the swap response
      expect(swapResponse.status).toBe(200);
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


  it('should execute a successful swap (ExactIn) with Lighthouse instructions', async () => {

    assert(params.WEBHOOK_ID, 'WEBHOOK_ID is not set');
    assert(params.TAKER_KEYPAIR, 'TAKER_KEYPAIR is not set');

    console.log(`how many ${params.MINT_A} will you get for ${params.AMOUNT} of ${params.MINT_B}?`);

    // Read the keypair
    const keypair: KeyPairSigner = await loadKeypairFromFile(params.TAKER_KEYPAIR);
    const taker = keypair.address;
    console.log('taker address: ', taker);

    const quoteURL = `${params.QUOTE_SERVICE_URL}/order`;
    console.log('request url: ', quoteURL);

    const quoteParams = {
      inputMint: params.MINT_B,
      outputMint: params.MINT_A,
      amount: `${params.AMOUNT}`,
      mode: "manual",
      swapMode: "ExactIn",
      slippageBps: 50,
      broadcastFeeType: "maxCap",
      priorityFeeLamports: 1000000,
      useWsol: false,
      asLegacyTransaction: false,
      excludeDexes: "",
      excludeRouters: "metis%2Chashflow%2Cdflow",
      taker: taker,
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
      expect(quoteResponse.data).toHaveProperty('maker');
      expect(quoteResponse.data.inAmount).toBe(`${params.AMOUNT}`);
      expect(quoteResponse.data.inputMint).toBe(params.MINT_B);
      expect(new BN(quoteResponse.data.outAmount).gt(new BN(0))).toBe(true);
      expect(quoteResponse.data.outputMint).toBe(params.MINT_A);


      // Step 2: Transaction signing
      const base64Transaction = quoteResponse.data.transaction;
      expect(base64Transaction).toBeDefined();

      let tx = createTransactionMessage({ version: 0 });

      const transactionBytes = getBase64Encoder().encode(base64Transaction);
      const transaction = getTransactionDecoder().decode(transactionBytes);
      const compiledTransactionMessage = getCompiledTransactionMessageDecoder().decode(transaction.messageBytes);

      // get the maker address
      console.log('transaction signatures: ', transaction.signatures);
      const maker = Object.keys(transaction.signatures).find((address) => address !== taker);
      // Add Lighthouse instructions to make sure the maker has at least 5,000,000 lamports
      const ix = getAssertAccountInfoInstruction({
        targetAccount: maker,
        assertion: accountInfoAssertion('Lamports', {
          value: 5_000_000,
          operator: IntegerOperator.GreaterThan,
        }),
      });

      const signedTransaction = await pipe(
        compiledTransactionMessage,
        (tx) => decompileTransactionMessage(tx),
        (tx) => appendTransactionMessageInstruction(ix, tx),
        (tx) => compileTransaction(tx),
        (tx) => partiallySignTransaction([keypair.keyPair], tx),
      );

      const signedTransactionBytes = getTransactionEncoder().encode(signedTransaction);
      const signedTransactionBase64 = getBase64Decoder().decode(signedTransactionBytes);

      // Step 3: Send the swap transaction
      const swapURL = `${params.QUOTE_SERVICE_URL}/execute`;
      const swapPayload = {
        requestId: quoteResponse.data.requestId,
        signedTransaction: signedTransactionBase64,
      };
      const swapParams = { swapType: 'rfq' };
      console.log("Swap payload --> ", swapPayload);

      const swapResponse = await axios.post(swapURL, swapPayload, { params: swapParams });
      console.log("Swap response --> ", swapResponse.data);

      // Assertions for the swap response
      expect(swapResponse.status).toBe(200);
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
