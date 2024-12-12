import { describe, it, expect } from 'vitest';
import axios from 'axios';
const BN = require('bn.js');


// Base API URL, load from environment variable or use default
const WEBHOOK_URL = process.env.WEBHOOK_URL || 'http://localhost:8080';

// Start mock server before tests and close it after
describe('Webhook API Quote', () => {
  it('should return a successful quote response', async () => {

    const url = `${WEBHOOK_URL}/quote`;
    console.log('request url: ', url);

    const payload = {
      amount: "250000000",
      amountIn: "250000000",
      protocol: "v1",
      quoteId: "59db3e19-c7b0-4753-a8aa-206701004498",
      quoteType: "exactIn",
      requestId: "629bddf3-0038-43a6-8956-f5433d6b1191",
      suggestedPrioritizationFees: 10000,
      taker: "5v2Vd71VoJ1wZhz1PkhTY48mrJwS6wF4LfvDbYPnJ3bc",
      tokenIn: "So11111111111111111111111111111111111111112",
      tokenOut: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
    }

    const response = await axios.post(url, payload);

    console.log("response --> ", response.data);

    expect(response.status).toBe(200);
    expect(response.data.quoteId).toBe(payload.quoteId);
    expect(response.data.requestId).toBe(payload.requestId);
    expect(response.data).toHaveProperty('maker');
    expect(response.data).toHaveProperty('amountOut');
    expect(new BN(response.data.amountOut).gt(new BN(0))).toBe(true);


  });



  it('should return a successful swap response', async () => {


    const url = `${WEBHOOK_URL}/swap`;
    console.log('request url: ', url);

    const payload = {
      quoteId: "59db3e19-c7b0-4753-a8aa-206701004498",
      requestId: "629bddf3-0038-43a6-8956-f5433d6b1191",
      transaction: "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAIABgxSnEehNb4kLTrfnzoVcTu/GPLBwP0kKZRTJowyLvHxxSdkl7oLuGWRcrcu3Yxm4Y9WF2TZyGphCjp+D3nAuvnbWolfQZ0Kl+9/uOLLKVXoXu/o/NQI5LY9pgx8ibLVfztqKpSdlIRAyuBnIsFa1A93abdI4AmIcbFLGFGatrhAXnMzpil7FnByGEuo10mEgCYqn/QfD1DTR6idALqAu9Bhh6NTL/nu9FDLsM2mMKzzPPKY2nBeuUHR7ibnmbqVw/MAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMGRm/lIRcy/+ytunLDm+e8jOW7xfcSayxDmzpAAAAABpuIV/6rgYT7aH9jRhjANdrEOdwa6ztVmKDwAAAAAAEG3fbh12Whk9nL4UbO63msHLSF7V9bN5E6jPWFfv8AqUpYSftyo7vpH9xbDmpX9jxaHLRbIGem7Qys02OVyKECxvp6877brTo9ZfNqq8l0MbG75MLS9uDkfKYCA0UvXWE9f/tHi80zsUphEh9edGz8h7JFCM8ITLeBpkq6CPTyfQMHAAkDmLEBAAAAAAAHAAUCwFwVAAoMAQACAwoFCwkICQYEIKhgt6NcCiigAMqaOwAAAAAm/0kBAQAAAEAvW2cAAAAAAA=="
    };

    const response = await axios.post(url, payload);

    console.log("response --> ", response.data);

    const SIGNATURE_LENGTH = 88; // 64 bytes encoded in base58

    expect(response.status).toBe(200);
    expect(response.data.quoteId).toBe(payload.quoteId);
    expect(response.data.state).toBe("accepted");
    expect(response.data.txSignature.length).toBe(SIGNATURE_LENGTH);
    // TODO verify the signature

  });


  // TODO: add failure test cases

});
