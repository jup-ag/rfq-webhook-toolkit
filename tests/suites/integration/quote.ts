import { describe, it, expect } from 'vitest';
import axios from 'axios';
import { assert } from 'chai';
const BN = require('bn.js');
const solanaWeb3 = require('@solana/web3.js');
const fs = require('fs');
import * as params from '../../params';


// Start mock server before tests and close it after
describe('Webhook e2e API Quote', () => {
  it('should return a successful quote response', async () => {

    assert(params.WEBHOOK_ID, 'WEBHOOK_ID is not set');
    assert(params.TAKER_KEYPAIR, 'TAKER_KEYPAIR is not set');

    // Read the keypair
    const keypair = solanaWeb3.Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(params.TAKER_KEYPAIR, 'utf8'))));
    const taker = keypair.publicKey.toString();
    console.log('taker address: ', taker);

    const url = `${params.QUOTE_SERVICE_URL}/quote`;
    console.log('request url: ', url);

    const payload = {
      swapMode: params.SWAP_MODE, // TODO: change to swapType
      taker: taker,
      inputMint: params.INPUT_MINT,
      outputMint: params.OUTPUT_MINT,
      amount: `${params.AMOUNT}`,
      feeBps: params.FEE_BPS,
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
      expect(response.data.orderInfo.input.token).toBe(params.INPUT_MINT);
      expect(new BN(response.data.orderInfo.output.startAmount).gt(new BN(0))).toBe(true);
      expect(response.data.orderInfo.output.token).toBe(params.OUTPUT_MINT);
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
