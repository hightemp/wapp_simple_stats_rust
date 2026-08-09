SHELL := /bin/bash
.DEFAULT_GOAL := help

CARGO ?= cargo
DEV_ADDRESS ?= 127.0.0.1
DEV_PORT ?= 8000

.PHONY: help config require-config dev watch run build release-check release \
	fmt fmt-check test lint check check-fast audit update deps docs clean

help: ## Show available targets
	@awk 'BEGIN {FS = ":.*## "; printf "Usage: make <target>\n\nTargets:\n"} /^[a-zA-Z_-]+:.*## / {printf "  %-16s %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@printf '\nDevelopment server options: DEV_ADDRESS=%s DEV_PORT=%s\n' "$(DEV_ADDRESS)" "$(DEV_PORT)"

config: ## Create config.yaml from the example without overwriting it
	@if [[ -e config.yaml ]]; then \
		printf '%s\n' 'config.yaml already exists'; \
	else \
		cp config.example.yaml config.yaml; \
		printf '%s\n' 'Created config.yaml — replace the example password before starting'; \
	fi

require-config:
	@if [[ ! -f config.yaml ]]; then \
		printf '%s\n' 'config.yaml was not found. Run make config and configure the password.' >&2; \
		exit 1; \
	fi

dev: require-config ## Start the development server (DEV_ADDRESS and DEV_PORT are configurable)
	ROCKET_ADDRESS="$(DEV_ADDRESS)" ROCKET_PORT="$(DEV_PORT)" $(CARGO) run --locked

watch: require-config ## Restart the development server on changes (requires cargo-watch)
	@command -v cargo-watch >/dev/null 2>&1 || { printf '%s\n' 'Install cargo-watch: cargo install cargo-watch --locked' >&2; exit 1; }
	ROCKET_ADDRESS="$(DEV_ADDRESS)" ROCKET_PORT="$(DEV_PORT)" cargo watch -x 'run --locked'

run: require-config ## Start the optimized release build
	ROCKET_ADDRESS="$(DEV_ADDRESS)" ROCKET_PORT="$(DEV_PORT)" $(CARGO) run --release --locked

build: ## Build the debug version
	$(CARGO) build --locked

release-check: ## Check release compilation
	$(CARGO) check --release --locked

release: ## Build the release binary
	$(CARGO) build --release --locked

fmt: ## Format Rust code
	$(CARGO) fmt

fmt-check: ## Check formatting without modifying files
	$(CARGO) fmt -- --check

test: ## Run tests
	$(CARGO) test --locked

lint: ## Run Clippy and treat warnings as errors
	$(CARGO) clippy --all-targets --all-features --locked -- -D warnings

check-fast: fmt-check ## Quickly check formatting and compilation
	$(CARGO) check --locked

check: fmt-check test lint ## Run all primary checks

audit: ## Audit dependencies via RustSec (requires cargo-audit)
	@command -v cargo-audit >/dev/null 2>&1 || { printf '%s\n' 'Install cargo-audit: cargo install cargo-audit --locked' >&2; exit 1; }
	$(CARGO) audit

update: ## Update compatible versions in Cargo.lock
	$(CARGO) update

deps: ## Show the direct dependency tree
	$(CARGO) tree -e normal --depth 1

docs: ## Build documentation without dependencies
	$(CARGO) doc --no-deps --locked

clean: ## Remove Cargo build artifacts
	$(CARGO) clean
