import axios from 'axios';
import { assert } from 'chai';
import { describe, expect, it } from 'vitest';
import * as params from '../../params';
const BN = require('bn.js');
const solanaWeb3 = require('@solana/web3.js');



// Base API URL, load from environment variable or use default
const WEBHOOK_URL = process.env.WEBHOOK_URL || 'http://localhost:8080';


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
describe('Webhook API Quote', () => {
  it('should return a successful quote response (ExactIn)', async () => {

    const url = `${WEBHOOK_URL}/quote`;
    console.log('request url: ', url);

    console.log(`how many ${params.MINT_A} will you get for ${params.AMOUNT} of ${params.MINT_B}?`);

    const payload = {
      amount: `${params.AMOUNT}`,
      feeBps: params.FEE_BPS,
      protocol: "v1",
      quoteId: "59db3e19-c7b0-4753-a8aa-206701004498",
      quoteType: "exactIn",
      requestId: "629bddf3-0038-43a6-8956-f5433d6b1191",
      suggestedPrioritizationFees: 10000,
      taker: "5v2Vd71VoJ1wZhz1PkhTY48mrJwS6wF4LfvDbYPnJ3bc",
      tokenIn: params.MINT_B,
      tokenOut: params.MINT_A
    }

    const response = await axios.post(url, payload).then((response) => {
      console.log("response --> ", response.data);

      expect(response.status).toBe(200);
      expect(response.data.quoteId).toBe(payload.quoteId);
      expect(response.data.requestId).toBe(payload.requestId);
      expect(response.data.tokenIn).toBe(payload.tokenIn);
      expect(response.data.tokenOut).toBe(payload.tokenOut);
      expect(response.data.quoteType).toBe(payload.quoteType);
      expect(response.data).toHaveProperty('maker');
      expect(response.data).toHaveProperty('amountOut');
      expect(response.data.taker).toBe(payload.taker);
      expect(response.data.amountIn).toBe(payload.amount);
      expect(new BN(response.data.amountOut).gt(new BN(0))).toBe(true);
    }).catch((error) => {
      if(error.response) {
        console.log("error.response.data --> ", error.response.data);
        assert.fail(`failed to get quote: unexpected response status ${error.response.status}: ${error.response.data}`);
      } else if(error.request) {
        console.log("error.request --> ", error.request.data);
        assert.fail('failed to get quote: no response from server');
      } else {
        console.log("error --> ", error);
        assert.fail('failed to get quote: unknown error');
      }
    });
  });

  it('should return a successful quote response (ExactOut)', async () => {

    const url = `${WEBHOOK_URL}/quote`;
    console.log('request url: ', url);

    console.log(`how many ${params.MINT_A} do you need to get ${params.AMOUNT} of ${params.MINT_B}?`);

    const payload = {
      amount: `${params.AMOUNT}`,
      feeBps: params.FEE_BPS,
      protocol: "v1",
      quoteId: "59db3e19-c7b0-4753-a8aa-206701004498",
      quoteType: "exactOut",
      requestId: "629bddf3-0038-43a6-8956-f5433d6b1191",
      suggestedPrioritizationFees: 10000,
      taker: "5v2Vd71VoJ1wZhz1PkhTY48mrJwS6wF4LfvDbYPnJ3bc",
      tokenIn: params.MINT_A,
      tokenOut: params.MINT_B
    }

    const response = await axios.post(url, payload).then((response) => {
      console.log("response --> ", response.data);

      expect(response.status).toBe(200);
      expect(response.data.quoteId).toBe(payload.quoteId);
      expect(response.data.requestId).toBe(payload.requestId);
      expect(response.data.tokenIn).toBe(payload.tokenIn);
      expect(response.data.tokenOut).toBe(payload.tokenOut);
      expect(response.data.quoteType).toBe(payload.quoteType);
      expect(response.data).toHaveProperty('maker');
      expect(response.data.taker).toBe(payload.taker);
      expect(response.data.amountOut).toBe(payload.amount);
      expect(new BN(response.data.amountIn).gt(new BN(0))).toBe(true);
    }).catch((error) => {
      if(error.response) {
        console.log("error.response.data --> ", error.response.data);
        assert.fail(`failed to get quote: unexpected response status ${error.response.status}: ${error.response.data}`);
      } else if(error.request) {
        console.log("error.request --> ", error.request.data);
        assert.fail('failed to get quote: no response from server');
      } else {
        console.log("error --> ", error);
        assert.fail('failed to get quote: unknown error');
      }
    });
  });

  it('should return a 404 for pair not supported', async () => {
    const url = `${WEBHOOK_URL}/quote`;
    console.log('request url: ', url);

    const payload = {
      amount: `${params.AMOUNT}`,
      feeBps: params.FEE_BPS,
      protocol: "v1",
      quoteId: "59db3e19-c7b0-4753-a8aa-206701004498",
      quoteType: "exactIn",
      requestId: "629bddf3-0038-43a6-8956-f5433d6b1191",
      suggestedPrioritizationFees: 10000,
      taker: "5v2Vd71VoJ1wZhz1PkhTY48mrJwS6wF4LfvDbYPnJ3bc",
      tokenIn: params.MINT_A,
      // this token does not exists so it cannot be supported and the response should be 404
      tokenOut: "fake3KUxqvJ5erXobKTYFtL2BpTgGzy7B9AcRcXeCwWvFM",
    }

    const response = await axios.post(url, payload).then((response) => {
      console.log("response --> ", response.data);
      assert.fail('expected 404 response');
    }).catch((error) => {
      if(error.response) {
        console.log("error.response.data --> ", error.response.data);
        expect(error.response.status).toBe(404);
      } else if(error.request) {
        console.log("error.request --> ", error.request.data);
        assert.fail('failed to get quote: no response from server');
      } else {
        console.log("error --> ", error);
        assert.fail('failed to get quote: unknown error');
      }
    });
  });

  it('should return a successful swap response', async () => {
    const url = `${WEBHOOK_URL}/swap`;
    console.log('request url: ', url);

    const payload = {
      quoteId: "59db3e19-c7b0-4753-a8aa-206701004498",
      requestId: "629bddf3-0038-43a6-8956-f5433d6b1191",
      transaction: "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAIABw1+jAiHYL/eHd3PMsF/IJuCQu5SqvEx+s2I0OosbQsG8kjAG1BZAFRV2dywxrzs3LT7Wy6rwamoK1c5K6qkDwTmwoAL86DDaPJrpECH4O7FIcjNK8aXLr8U+vEPOkKqMIbT6oz1rKyozQUgdRIXXEPO9Upd2Z7eIKFrVSU3OPOX3N7E3kRk8Ll8XsOf5Ir4ISzHf+0ZUtqBSXSNVE5iS+sA4iF2IlhNfbkvqPIGGddbql5WIVIAOvUkFwCrBoXw04EAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMGRm/lIRcy/+ytunLDm+e8jOW7xfcSayxDmzpAAAAABHnZx8wQNd5yEfmetIwJ1wsr31vfni5WuKH7taLqMycG3fbh12Whk9nL4UbO63msHLSF7V9bN5E6jPWFfv8AqUpYSftyo7vpH9xbDmpX9jxaHLRbIGem7Qys02OVyKECjJclj04kifG7PRApFI4NgwtaE5na/xCEBI572Nvp+FnG+nrzvtutOj1l82qryXQxsbvkwtL24OR8pgIDRS9dYSdenhhyIvZ+yaOk0Giv3sQzHPEybygyONx7iDX7IHF2BAcACQMK0gAAAAAAAAcABQIdmAAACwYBBAEMBgkBAQoLAQACBQQDCAkMCQYjqGC3o1wKKKAAypo7AAAAAJgWfQEAAAAAaiqpZwAAAAAKAAAA"
    };

    const response = await axios.post(url, payload);

    console.log("response --> ", response.data);

    const SIGNATURE_LENGTH = 88; // 64 bytes encoded in base58

    expect(response.status).toBe(200);
    expect(response.data.quoteId).toBe(payload.quoteId);
    expect(response.data.state).toBe("accepted");
  });

  it('it should simulate a swap rejection', async () => {
    const url = `${WEBHOOK_URL}/swap`;
    console.log('request url: ', url);

    const payload = {
      quoteId: "59db3e19-c7b0-4753-a8aa-206701004498",
      requestId: "00000000-0000-0000-0000-000000000001", // special requestId to simulate rejection
      transaction: "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAIABw1+jAiHYL/eHd3PMsF/IJuCQu5SqvEx+s2I0OosbQsG8kjAG1BZAFRV2dywxrzs3LT7Wy6rwamoK1c5K6qkDwTmwoAL86DDaPJrpECH4O7FIcjNK8aXLr8U+vEPOkKqMIbT6oz1rKyozQUgdRIXXEPO9Upd2Z7eIKFrVSU3OPOX3N7E3kRk8Ll8XsOf5Ir4ISzHf+0ZUtqBSXSNVE5iS+sA4iF2IlhNfbkvqPIGGddbql5WIVIAOvUkFwCrBoXw04EAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMGRm/lIRcy/+ytunLDm+e8jOW7xfcSayxDmzpAAAAABHnZx8wQNd5yEfmetIwJ1wsr31vfni5WuKH7taLqMycG3fbh12Whk9nL4UbO63msHLSF7V9bN5E6jPWFfv8AqUpYSftyo7vpH9xbDmpX9jxaHLRbIGem7Qys02OVyKECjJclj04kifG7PRApFI4NgwtaE5na/xCEBI572Nvp+FnG+nrzvtutOj1l82qryXQxsbvkwtL24OR8pgIDRS9dYSdenhhyIvZ+yaOk0Giv3sQzHPEybygyONx7iDX7IHF2BAcACQMK0gAAAAAAAAcABQIdmAAACwYBBAEMBgkBAQoLAQACBQQDCAkMCQYjqGC3o1wKKKAAypo7AAAAAJgWfQEAAAAAaiqpZwAAAAAKAAAA"
    };

    const response = await axios.post(url, payload);

    console.log("response --> ", response.data);

    expect(response.status).toBe(200);
    expect(response.data.quoteId).toBe(payload.quoteId);
    expect(response.data.state).toBe("rejected");
    expect(response.data).toHaveProperty('rejectionReason');
    expect(response.data.rejectionReason).toBeTruthy();
  });


  it('should return a successful accepted token list', async () => {
    const url = `${WEBHOOK_URL}/tokens`;
    console.log('request url: ', url);

    const response = await axios.get(url);

    console.log("response --> ", response.data);

    expect(response.status).toBe(200);
    expect(response.data.length).toBeGreaterThanOrEqual(0);

    for (let tokenAddress of response.data) {
      expect(isValidSolanaAddress(tokenAddress)).toBe(true);
    }

  });


});
