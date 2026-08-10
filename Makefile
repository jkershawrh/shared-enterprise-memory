PODMAN ?= podman
POD_NAME = projections-dev
PG_CONTAINER = projections-pg
APP_CONTAINER = projections-app
PG_IMAGE = docker.io/library/postgres:16-bookworm
APP_IMAGE = localhost/projections:dev
DB_NAME = projections_dev
DB_USER = projections
DB_PASS = projections_dev_pass
DB_PORT = 15432
API_PORT = 18084
METRICS_PORT = 18085
DB_URL = host=127.0.0.1 port=$(DB_PORT) user=$(DB_USER) password=$(DB_PASS) dbname=$(DB_NAME)

.PHONY: help build up down pg migrate run smoke clean test fmt lint check

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}'

build: ## Build container image
	$(PODMAN) build -t $(APP_IMAGE) -f Containerfile .

pg: ## Start PostgreSQL container
	@$(PODMAN) container exists $(PG_CONTAINER) 2>/dev/null && echo "PostgreSQL already running" || \
	$(PODMAN) run -d --name $(PG_CONTAINER) \
		-e POSTGRES_DB=$(DB_NAME) \
		-e POSTGRES_USER=$(DB_USER) \
		-e POSTGRES_PASSWORD=$(DB_PASS) \
		-p $(DB_PORT):5432 \
		$(PG_IMAGE)
	@echo "Waiting for PostgreSQL..."
	@for i in $$(seq 1 30); do \
		$(PODMAN) exec $(PG_CONTAINER) pg_isready -U $(DB_USER) -d $(DB_NAME) >/dev/null 2>&1 && break; \
		sleep 1; \
	done
	@echo "PostgreSQL ready on port $(DB_PORT)"

migrate: pg ## Apply migrations
	$(PODMAN) exec -i $(PG_CONTAINER) psql -U $(DB_USER) -d $(DB_NAME) < migrations/001_projection_schema.sql
	@echo "Migration applied"

up: migrate ## Start PostgreSQL + apply migrations
	@echo "Database ready at: $(DB_URL)"

run: up ## Run projections service locally (cargo)
	PROJECTION_DB_CONNECTION_STRING="$(DB_URL)" \
	PROJECTION_CONTEXT_API_PORT=$(API_PORT) \
	PROJECTION_METRICS_PORT=$(METRICS_PORT) \
	cargo run

down: ## Stop and remove containers
	-$(PODMAN) rm -f $(APP_CONTAINER) 2>/dev/null
	-$(PODMAN) rm -f $(PG_CONTAINER) 2>/dev/null

clean: down ## Stop containers and remove image
	-$(PODMAN) rmi $(APP_IMAGE) 2>/dev/null

smoke: up ## Run smoke test against running service
	@echo "=== Smoke Test ==="
	@echo "1. Check health..."
	@curl -sf http://localhost:$(API_PORT)/healthz && echo " OK" || echo " FAIL (is the service running? try 'make run' first)"
	@echo "2. Check readiness..."
	@curl -sf http://localhost:$(API_PORT)/readyz && echo " OK" || echo " FAIL"
	@echo "3. Check metrics..."
	@curl -sf http://localhost:$(METRICS_PORT)/metrics | head -3 && echo "... OK" || echo " FAIL"
	@echo "4. Query context (empty)..."
	@curl -sf -H "X-Agent-ID: test-agent" http://localhost:$(API_PORT)/v1/memory/context/acme/incident-42 | python3 -m json.tool 2>/dev/null || echo "FAIL"
	@echo "5. Query status..."
	@curl -sf http://localhost:$(API_PORT)/v1/memory/status | python3 -m json.tool 2>/dev/null || echo "FAIL"
	@echo "=== Smoke Test Complete ==="

test: ## Run all Rust tests
	cargo test --all

fmt: ## Check formatting
	cargo fmt --all --check

lint: ## Run clippy
	cargo clippy --all-targets --all-features -- -D warnings

check: fmt lint test ## Run fmt + lint + test
