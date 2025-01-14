.PHONY: run-server-example

run-example-server:
	cargo run --package server-example


prepare-tests:
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
	  WEBHOOK_URL=$${WEBHOOK_URL:-http://localhost:8080}; \
	  echo "Using WEBHOOK_URL=$$WEBHOOK_URL"; \
	  timeout=120; \
	  elapsed=0; \
	  while ! curl -s -o /dev/null -w "%{http_code}" $$WEBHOOK_URL/health | grep -q "200"; do \
	    if [ $$elapsed -ge $$timeout ]; then \
	      echo "Server health check timed out after $$timeout seconds"; \
	      kill $$server_pid; \
	      exit 1; \
	    fi; \
	    echo "Waiting for $$WEBHOOK_URL/health to return 200... ($$elapsed seconds elapsed)"; \
	    sleep 1; \
	    elapsed=$$((elapsed + 1)); \
	  done; \
	  echo "Health endpoint is ready! Running acceptance tests"; \
	  make run-acceptance-tests; \
	)


run-integration-tests:
	cd tests && pnpm run integration


update-openapi-spec:
	@echo "Running the example server to update the openapi docs" && \
	( \
	  echo "Starting the example server"; \
	  make run-example-server & \
	  server_pid=$$!; \
	  trap 'kill $$server_pid' EXIT INT TERM; \
	  WEBHOOK_URL=$${WEBHOOK_URL:-http://localhost:8080}; \
	  echo "Using WEBHOOK_URL=$$WEBHOOK_URL"; \
	  timeout=120; \
	  elapsed=0; \
	  while ! curl -s -o /dev/null -w "%{http_code}" $$WEBHOOK_URL/health | grep -q "200"; do \
	    if [ $$elapsed -ge $$timeout ]; then \
	      echo "Server health check timed out after $$timeout seconds"; \
	      kill $$server_pid; \
	      exit 1; \
	    fi; \
	    echo "Waiting for $$WEBHOOK_URL/health to return 200... ($$elapsed seconds elapsed)"; \
	    sleep 1; \
	    elapsed=$$((elapsed + 1)); \
	  done; \
	  echo "Health endpoint is ready! fetching the specs"; \
	  curl -s $$WEBHOOK_URL/api-doc/openapi.json | jq > openapi/openapi.json; \
	)