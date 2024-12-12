.PHONY: run-server-example

run-example-server:
	cargo run --package server-example


prepare-acceptance-tests:
	cd tests && pnpm install

run-acceptance-tests:
	cd tests && pnpm run acceptance

# run acceptance tests with the example server
run-acceptance-tests-against-sample-server:
	@echo "Running acceptance tests with the example server" && \
	( \
	  echo "Starting the example server"; \
	  make run-example-server & \
	  server_pid=$$!; \
	  trap 'kill $$server_pid' EXIT INT TERM; \
	  echo "Waiting for the server to start"; \
	  sleep 1; \
	  echo "Running acceptance tests"; \
	  make run-acceptance-tests; \
	)
