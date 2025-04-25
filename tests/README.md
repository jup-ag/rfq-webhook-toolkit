# Tests

This directory contains tests for the webhook API. The tests are written using [Vitest](https://vitest.dev).


## Requirements

To run the tests, you need to have the following installed:

- [Node.js](https://nodejs.org/en/download/)
- [Vitest](https://vitest.dev)
- [pnpm](https://pnpm.io)


## Suites

Two suites are available for testing the webhook API, acceptance and integration tests.

To install the dependencies for the tests, run:

```bash
make prepare-tests
```

### Acceptance tests

These tests simulate the interaction between the Jupiter RFQ module and the webhook API. The tests are useful to verify that your implementation is compatible with the Jupiter RFQ module,

The tests can be found in the [`acceptance`](./tests/suites/acceptance/) directory.

To run the tests, you need to provide the webhook URL:

```bash
WEBHOOK_URL=<your_webhook_url> make run-acceptance-tests
```

you can also provide an api key if your webhook requires it:

```bash
WEBHOOK_URL=<your_webhook_url> WEBHOOK_API_KEY=<your_webhook_api_key> make run-acceptance-tests
```

for an example, you can run the tests against the bundled [sample server](../server-example/):

```bash
make run-acceptance-tests-against-sample-server
```

To run the test directly with `pnpm` run:

```sh
pnpm run acceptance
```

### Integration tests

Integration tests are end to end tests that simulate the user interaction. The tests are running against our edge (pre-production) environment and require that the webhook has been registered with Jupiter RFQ.

The tests can be found in the [`integration`](./tests/suites/integration/) directory.

```sh
TAKER_KEYPAIR=<path_to_your_keypair.json> \
WEBHOOK_ID=<your_webhook_id> \
make run-integration-tests
```

By default, the tests will attempt to get a quote for 1 USDC to SOL, upon a successful quote, the tests will proceed to swap the tokens. To modify the parameters of the quote and swap, you can override the environment variables defined in the [`params.ts`](./params.ts) file.

> :warning: **Warning**: Running the integration tests will perform a real swap on the Solana mainnet. Make sure you have the necessary funds in your wallet before running the tests.


To run the test directly with `pnpm` run:

```sh
pnpm run integration
```


### Manual tests

To test a webhook live on EDGE, you can use a tool such as [Postman](https://www.postman.com/) or `curl`. See the following examples:

**Do not forget to replace TAKER_PUBLIC_ADDRESS and YOUR_WEBHOOK_ID in the url**

<details> <summary><strong>cURL</strong></summary>

```bash
curl --location 'https://rfq-api-edge.raccoons.dev/v1/quote?inputMint=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v&outputMint=So11111111111111111111111111111111111111112&amount=1000000&taker=<TAKER_PUBLIC_ADDRESS>&quoteType=exactIn&version=v1&webhookId=<YOUR_WEBHOOK_ID>&swapType=rfq' --header 'Content-Type: application/json'
```
</details> <details> <summary><strong>Rust</strong></summary>

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .build()?;

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse()?);

    let data = "";

    let request = client.request(reqwest::Method::GET, "https://rfq-api-edge.raccoons.dev/v1/quote?inputMint=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v&outputMint=So11111111111111111111111111111111111111112&amount=1000000&taker=TAKER_PUBLIC_KEY&quoteType=exactIn&version=v1&webhookId=YOUR_WEBHOOK_ID&swapType=rfq")
        .headers(headers)
        .body(data);

    let response = request.send().await?;
    let body = response.text().await?;

    println!("{}", body);

    Ok(())
}
```
</details> <details> <summary><strong>Python</strong></summary>

```python
import requests
import json

url = "https://rfq-api-edge.raccoons.dev/v1/quote?inputMint=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v&outputMint=So11111111111111111111111111111111111111112&amount=1000000&taker=TAKER_PUBLIC_KEY&quoteType=exactIn&version=v1&webhookId=YOUR_WEBHOOK_ID&swapType=rfq"

payload = ""
headers = {
  'Content-Type': 'application/json'
}

response = requests.request("GET", url, headers=headers, data=payload)

print(response.text)
```
</details> <details> <summary><strong>TypeScript</strong></summary>

```typescript
var request = require('request');
var options = {
  'method': 'GET',
  'url': 'https://rfq-api-edge.raccoons.dev/v1/quote?inputMint=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v&outputMint=So11111111111111111111111111111111111111112&amount=1000000&taker=TAKER_PUBLIC_KEY&quoteType=exactIn&version=v1&webhookId=YOUR_WEBHOOK_ID&swapType=rfq',
  'headers': {
    'Content-Type': 'application/json'
  }
};
request(options, function (error, response) {
  if (error) throw new Error(error);
  console.log(response.body);
});
```

</details>

## Troubleshooting

#### Requests do not reach the webhook during integration tests
The most likely cause is a request timeout. Check the timeout requirements [here](../README.md#timeouts).  

#### The webhook provides a quote, but the RFQ returns a 404
The most probable cause is that the quote **fails simulation**. Every quote is simulated, and those that fail **are not propagated to the frontend**. The most common reasons for simulation failure are:  

1. The Maker does not have enough inventory for the swap.  
2. The Maker does not have enough SOL to cover network fees.  
3. The Maker does not have an **Associated Token Account (ATA)** for either the input or output mint.
4. The Taker does not have enough funds for the swap.  

Regarding point **#3**, the Maker is required to have the **ATA configured** for all tokens it advertises; SOL are automatically wrapped by the Order Engine program, the Maker must have the ATA for WSOL as well to proecess SOL swaps.  

#### The webhook returns the best quote, but it is not the one presented to the user
See the section ["The webhook provides a quote, but the RFQ returns a 404"](#the-webhook-provides-a-quote-but-the-rfq-returns-404).  
