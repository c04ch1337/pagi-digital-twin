.PHONY: run-dev stop-dev run-legacy-dev stop-legacy-dev test docker-up docker-down docker-generate

run-dev:
	python scripts/run_all_dev.py --profile core

stop-dev:
	python scripts/run_all_dev.py --stop

# Legacy bare-metal harness (BFF + Python Agent demo). Not required for the
# current UI-agnostic core stack (Agent Planner).
run-legacy-dev:
	python scripts/run_all_dev.py --profile legacy

stop-legacy-dev:
	python scripts/run_all_dev.py --profile legacy --stop

test:
	@echo "No tests yet. Add per-service tests as you implement logic."
	@echo "Suggested: Python pytest, Go test ./..., Rust cargo test."

docker-up:
	docker compose up --build

docker-down:
	docker compose down

docker-generate:
	# Utility target to generate gRPC code for bare metal testing
	go generate ./backend-go-model-gateway/...
