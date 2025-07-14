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
  timeout: 60_000, // Increased to 60 seconds
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
      excludeRouters: "metis,hashflow,dflow,pyth,okx",
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
      expect(swapResponse.data.status).toBe("Success");

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
      excludeRouters: "metis,hashflow,dflow,pyth,okx",
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
      expect(swapResponse.data.status).toBe("Success");

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
      priorityFeeLamports: 2000000, // Increased priority fee for faster processing
      useWsol: false,
      asLegacyTransaction: false,
      excludeDexes: "",
      excludeRouters: "metis,hashflow,dflow,pyth,okx",
      taker: taker,
      webhookId: params.WEBHOOK_ID,
    }

    try {
      console.log("Step 1: Fetching quote...");
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

      console.log("Step 2: Processing transaction...");
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
      assert(maker, 'Maker address not found in transaction signatures');

      // Decompile the transaction to work with it
      let decompiledTx = decompileTransactionMessage(compiledTransactionMessage);

      // Find and log existing compute budget instructions
      const computeBudgetProgramId = 'ComputeBudget111111111111111111111111111111';

      console.log("Checking existing compute budget instructions...");
      let foundComputeUnitLimit = false;

      decompiledTx.instructions.forEach((instruction, index) => {
        if (instruction.programAddress === computeBudgetProgramId && instruction.data) {
          console.log(`Found compute budget instruction at index ${index}, data:`, Array.from(instruction.data));
          if (instruction.data[0] === 2) {
            const currentLimit = new DataView(instruction.data.buffer, 1).getUint32(0, true);
            console.log(`Current compute unit limit: ${currentLimit}`);
            foundComputeUnitLimit = true;
          }
        }
      });

      // Try to create a new instruction list with modified compute unit limit
      let newInstructions = [...decompiledTx.instructions];

      if (foundComputeUnitLimit) {
        console.log("Attempting to increase compute unit limit...");

        // Find and replace the compute unit limit instruction
        newInstructions = decompiledTx.instructions.map((instruction) => {
          if (instruction.programAddress === computeBudgetProgramId &&
            instruction.data &&
            instruction.data[0] === 2) {

            // Create new compute unit limit instruction with 400,000 units
            const newLimit = 400000;
            const limitBytes = new Uint8Array(4);
            new DataView(limitBytes.buffer).setUint32(0, newLimit, true);

            console.log(`Replacing compute unit limit with ${newLimit}`);
            return {
              programAddress: computeBudgetProgramId as any,
              accounts: instruction.accounts,
              data: new Uint8Array([2, ...limitBytes])
            };
          }
          return instruction;
        });

        // Create new transaction message with modified instructions
        decompiledTx = {
          ...decompiledTx,
          instructions: newInstructions as any
        };
      } else {
        console.log("No compute unit limit instruction found");
      }

      // Add a much simpler Lighthouse instruction that uses fewer compute units
      const lighthouseInstruction = getAssertAccountInfoInstruction({
        targetAccount: maker as any,
        assertion: accountInfoAssertion('Lamports', {
          value: 5000000,
          operator: IntegerOperator.GreaterThan,
        }),
      });

      console.log("Building signed transaction with simple lighthouse instruction...");
      const signedTransaction = await pipe(
        decompiledTx,
        (tx) => appendTransactionMessageInstruction(lighthouseInstruction, tx),
        (tx) => compileTransaction(tx),
        (tx) => partiallySignTransaction([keypair.keyPair], tx),
      );

      const signedTransactionBytes = getTransactionEncoder().encode(signedTransaction);
      const signedTransactionBase64 = getBase64Decoder().decode(signedTransactionBytes);

      console.log("Step 3: Executing swap...");
      // Step 3: Send the swap transaction
      const swapURL = `${params.QUOTE_SERVICE_URL}/execute`;
      const swapPayload = {
        requestId: quoteResponse.data.requestId,
        signedTransaction: signedTransactionBase64,
      };
      const swapParams = { swapType: 'rfq' };
      console.log("Swap payload --> ", swapPayload);

      const swapResponse = await axios.post(swapURL, swapPayload, {
        params: swapParams,
        timeout: 45000 // 45 second timeout for the swap execution
      });
      console.log("Swap response --> ", swapResponse.data);

      // Assertions for the swap response
      expect(swapResponse.status).toBe(200);
      expect(swapResponse.data.status).toBe("Success");

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
  }, 50000); // 50 second timeout for this specific test
});
