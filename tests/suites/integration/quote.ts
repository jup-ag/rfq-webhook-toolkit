import axios from 'axios';
import { assert } from 'chai';
import { describe, expect, it } from 'vitest';
import * as params from '../../params';
import {loadKeypairFromFile} from '../../helpers';
import {BN} from 'bn.js';

// Start mock server before tests and close it after
describe('Webhook e2e API Quote', () => {
  it('should return a successful quote response (ExactIn)', async () => {

    assert(params.WEBHOOK_ID, 'WEBHOOK_ID is not set');
    assert(params.TAKER_KEYPAIR, 'TAKER_KEYPAIR is not set');

    console.log(`how many ${params.MINT_A} will you get for ${params.AMOUNT} of ${params.MINT_B}?`);

    // Read the keypair
    const keypair = await loadKeypairFromFile(params.TAKER_KEYPAIR);
    const taker = keypair.address;
    console.log('taker address: ', taker);

    const url = `${params.QUOTE_SERVICE_URL}/quote`;
    console.log('request url: ', url);

    const payload = {
      swapMode: "exactIn",
      taker: taker,
      inputMint: params.MINT_B,
      outputMint: params.MINT_A,
      amount: `${params.AMOUNT}`,
      swapType: 'rfq',
      webhookId: params.WEBHOOK_ID,
    }

    await axios.get(url, { params: payload })
    .then((response) => {
      console.log("response --> ", response.data);
      expect(response.status).toBe(200);
      expect(response.data).toHaveProperty('quoteId');
      expect(response.data).toHaveProperty('requestId');
      expect(response.data).toHaveProperty('expireAt');
      expect(response.data).toHaveProperty('orderInfo');
      expect(response.data).toHaveProperty('maker');
      expect(response.data).toHaveProperty('orderInfo');
      expect(response.data.swapMode).toBe(payload.swapMode);
      expect(response.data.orderInfo.input.startAmount).toBe(`${params.AMOUNT}`);
      expect(response.data.orderInfo.input.token).toBe(params.MINT_B);
      expect(new BN(response.data.orderInfo.output.endAmount).gt(new BN(0))).toBe(true);
      expect(response.data.orderInfo.output.token).toBe(params.MINT_A);
    })
    .catch((error) => {
      if(error.response) {
        console.log("error.response.data --> ", error.response.data);
        assert.fail(`failed to get quote: unexpected response status ${error.response.status}: ${error.response.data.error}`);
      } else if(error.request) {
        assert.fail(`failed to get quote: no response from server ${error.config.url}`);
      } else {
        console.log("error --> ", error);
        assert.fail(`failed to get quote: unknown error for ${error.config.url}`);
      }
    });
  });

  it('should return a successful quote response (ExactOut)', async () => {

    assert(params.WEBHOOK_ID, 'WEBHOOK_ID is not set');
    assert(params.TAKER_KEYPAIR, 'TAKER_KEYPAIR is not set');

    console.log(`how many ${params.MINT_A} do you need to get ${params.AMOUNT} of ${params.MINT_B}?`);

    // Read the keypair
    const keypair = await loadKeypairFromFile(params.TAKER_KEYPAIR);
    const taker = keypair.address;
    console.log('taker address: ', taker);

    const url = `${params.QUOTE_SERVICE_URL}/quote`;
    console.log('request url: ', url);

    const payload = {
      swapMode: "exactOut",
      taker: taker,
      inputMint: params.MINT_A,
      outputMint: params.MINT_B,
      amount: `${params.AMOUNT}`,
      swapType: 'rfq',
      webhookId: params.WEBHOOK_ID,
    }

    const response = await axios.get(url, { params: payload })
    .then((response) => {
      console.log("response --> ", response.data);
      expect(response.status).toBe(200);
      expect(response.data).toHaveProperty('quoteId');
      expect(response.data).toHaveProperty('requestId');
      expect(response.data).toHaveProperty('expireAt');
      expect(response.data).toHaveProperty('orderInfo');
      expect(response.data).toHaveProperty('maker');
      expect(response.data).toHaveProperty('orderInfo');
      expect(response.data.swapMode).toBe(payload.swapMode);
      expect(new BN(response.data.orderInfo.input.endAmount).gt(new BN(0))).toBe(true);
      expect(response.data.orderInfo.input.token).toBe(params.MINT_A);
      expect(response.data.orderInfo.output.endAmount).toBe(`${params.AMOUNT}`);
      expect(response.data.orderInfo.output.token).toBe(params.MINT_B);
    })
    .catch((error) => {
      if(error.response) {
        console.log("error.response.data --> ", error.response.data);
        assert.fail(`failed to get quote: unexpected response status ${error.response.status}: ${error.response.data.error}`);
      } else if(error.request) {
        assert.fail(`failed to get quote: no response from server ${error.config.url}`);
      } else {
        console.log("error --> ", error);
        assert.fail(`failed to get quote: unknown error for ${error.config.url}`);
      }
    });
  });

  it('should return a successful quote response (with an empty taker) - ExactIn', async () => {
    // in this test the taker is not set because the user has not connected the wallet
    assert(params.WEBHOOK_ID, 'WEBHOOK_ID is not set');

    console.log(`how many ${params.MINT_A} can you get for ${params.AMOUNT} of ${params.MINT_B}?`);

    const url = `${params.QUOTE_SERVICE_URL}/quote`;
    console.log('request url: ', url);

    const payload = {
      inputMint: params.MINT_B,
      outputMint: params.MINT_A,
      amount: `${params.AMOUNT}`,
      swapType: 'rfq',
      webhookId: params.WEBHOOK_ID,
    }

    const response = await axios.get(url, { params: payload })
    .then((response) => {
      console.log("response --> ", response.data);
      expect(response.status).toBe(200);
      expect(response.data).toHaveProperty('quoteId');
      expect(response.data).toHaveProperty('requestId');
      expect(response.data).toHaveProperty('expireAt');
      expect(response.data).toHaveProperty('orderInfo');
      expect(response.data).toHaveProperty('maker'); // the maker should be the MM address
      expect(response.data).toHaveProperty('orderInfo');
      expect(response.data.orderInfo.input.startAmount).toBe(`${params.AMOUNT}`);
      expect(response.data.orderInfo.input.token).toBe(params.MINT_B);
      expect(new BN(response.data.orderInfo.output.startAmount).gt(new BN(0))).toBe(true);
      expect(response.data.orderInfo.output.token).toBe(params.MINT_A);
    })
    .catch((error) => {
      if(error.response) {
        console.log("error.response.data --> ", error.response.data);
        assert.fail(`failed to get quote: unexpected response status ${error.response.status}: ${error.response.data.error}`);
      } else if(error.request) {
        assert.fail(`failed to get quote: no response from server ${error.config.url}`);
      } else {
        console.log("error --> ", error);
        assert.fail(`failed to get quote: unknown error for ${error.config.url}`);
      }
    });
  });
});
