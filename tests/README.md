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

To test a webhook via the [edge UI](https://edge.jup.ag) with you can use a browser extension ([example](https://chromewebstore.google.com/search/Inssman)) that allows to modify http request params, adding the rules:

- host: `https://quote-proxy-edge.raccoons.dev/*`
- param: `webhookID=<your_webhook_id>`
