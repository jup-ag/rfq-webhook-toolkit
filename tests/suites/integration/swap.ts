import { describe, it, expect } from 'vitest';
import axios from 'axios';
import { assert } from 'chai';
const BN = require('bn.js');
const solanaWeb3 = require('@solana/web3.js');
const fs = require('fs');
import * as params from '../../params';


// Start mock server before tests and close it after
describe('Webhook EDGE API Swap', () => {
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

    let response = await axios.get(quoteURL, { params: quoteParams });

    console.log("response --> ", response.data);

    expect(response.status).toBe(200);
    expect(response.data).toHaveProperty('quoteId');
    expect(response.data).toHaveProperty('requestId');
    expect(response.data).toHaveProperty('expireAt');
    expect(response.data).toHaveProperty('orderInfo');
    expect(response.data).toHaveProperty('maker'); // the maker should be the MM address
    expect(response.data).toHaveProperty('orderInfo');
    expect(response.data.orderInfo.input.startAmount).toBe(`${params.AMOUNT}`);
    expect(response.data.orderInfo.input.token).toBe(params.INPUT_MINT);
    expect(new BN(response.data.orderInfo.output.startAmount).gt(new BN(0))).toBe(true);
    expect(response.data.orderInfo.output.token).toBe(params.OUTPUT_MINT);


    const base64Transaction = response.data.transaction;
    expect(base64Transaction).toBeDefined();
    // sign the transaction

    const transactionBytes = Buffer.from(base64Transaction, 'base64');

    // Deserialize the transaction
    const transaction = solanaWeb3.VersionedTransaction.deserialize(transactionBytes);

    // Sign the transaction
    transaction.sign([keypair]);
    const signedTransactionBase64 = Buffer.from(transaction.serialize()).toString('base64');

    // Send the swap transaction
    const swapURL = `${params.QUOTE_SERVICE_URL}/swap`;

    const swapPayload = {
      quoteId: response.data.quoteId,
      requestId: response.data.requestId,
      transaction: signedTransactionBase64
    };

    const swapParams = {
      swapType: 'rfq',
    };

    console.log(swapPayload);

    response = await axios.post(swapURL, swapPayload, { params: swapParams });

    console.log("response --> ", response.data);
    expect(response.status).toBe(200);
    expect(response.data.quoteId).toBe(swapPayload.quoteId);
    expect(response.data.state).toBe("accepted");

  });
});
