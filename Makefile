# ==============================================================================
# KIAS - Kubernetes-like Intelligent Agent Scheduling System
# Makefile
# ==============================================================================

# Colors
GREEN  := \033[0;32m
YELLOW := \033[0;33m
RED    := \033[0;31m
BLUE   := \033[0;34m
CYAN   := \033[0;36m
BOLD   := \033[1m
RESET  := \033[0m

# Default target
.DEFAULT_GOAL := help

# ==============================================================================
# Phony targets
# ==============================================================================
.PHONY: all build release test test-verbose check clippy fmt fmt-check lint lint-arch ci clean doc
.PHONY: run-api run-monitor test-api test-scheduler test-controller
.PHONY: count bench help

# ==============================================================================
# Build targets
# ==============================================================================

## Build all crates (debug)
build:
	@printf "$(BLUE)$(BOLD)▶ Building (debug)...$(RESET)\n"
	cargo build

## Build all crates (release)
release:
	@printf "$(BLUE)$(BOLD)▶ Building (release)...$(RESET)\n"
	cargo build --release

# ==============================================================================
# Test targets
# ==============================================================================

## Run all tests
test:
	@printf "$(GREEN)$(BOLD)▶ Running tests...$(RESET)\n"
	cargo test

## Run all tests with output
test-verbose:
	@printf "$(GREEN)$(BOLD)▶ Running tests (verbose)...$(RESET)\n"
	cargo test -- --nocapture

## Run API server tests
test-api:
	@printf "$(GREEN)$(BOLD)▶ Testing kias-api-server...$(RESET)\n"
	cargo test -p kias-api-server

## Run scheduler tests
test-scheduler:
	@printf "$(GREEN)$(BOLD)▶ Testing kias-scheduler...$(RESET)\n"
	cargo test -p kias-scheduler

## Run controller tests
test-controller:
	@printf "$(GREEN)$(BOLD)▶ Testing kias-controller...$(RESET)\n"
	cargo test -p kias-controller

# ==============================================================================
# Quality targets
# ==============================================================================

## Run cargo check
check:
	@printf "$(CYAN)$(BOLD)▶ Checking...$(RESET)\n"
	cargo check

## Run clippy with warnings as errors
clippy:
	@printf "$(YELLOW)$(BOLD)▶ Running clippy...$(RESET)\n"
	cargo clippy -- -D warnings

## Format code
fmt:
	@printf "$(GREEN)$(BOLD)▶ Formatting...$(RESET)\n"
	cargo fmt

## Check formatting (CI-friendly)
fmt-check:
	@printf "$(YELLOW)$(BOLD)▶ Checking formatting...$(RESET)\n"
	cargo fmt --check

## Run fmt-check + clippy
lint: fmt-check clippy
	@printf "$(GREEN)$(BOLD)✔ Lint passed$(RESET)\n"

## Check architecture layer dependencies (L0←L1←L2←L3) via cargo metadata
lint-arch:
	@printf "$(YELLOW)$(BOLD)▶ Checking architecture layers (cargo metadata)...$(RESET)\n"
	@python3 scripts/lint-arch.py

# ==============================================================================
# Run targets
# ==============================================================================

## Run API server
run-api:
	@printf "$(BLUE)$(BOLD)▶ Starting kias-api-server...$(RESET)\n"
	cargo run -p kias-api-server

## Run monitor
run-monitor:
	@printf "$(BLUE)$(BOLD)▶ Starting kias-monitor...$(RESET)\n"
	cargo run -p kias-monitor

# ==============================================================================
# Utility targets
# ==============================================================================

## Clean build artifacts
clean:
	@printf "$(RED)$(BOLD)▶ Cleaning...$(RESET)\n"
	cargo clean

## Generate and open documentation
doc:
	@printf "$(BLUE)$(BOLD)▶ Generating docs...$(RESET)\n"
	cargo doc --open

## Run benchmarks
bench:
	@printf "$(CYAN)$(BOLD)▶ Running benchmarks...$(RESET)\n"
	cargo bench -p kias-benchmarks -- --output-format bencher | tee /tmp/kias-bench.txt
	@printf "$(GREEN)$(BOLD)✔ Benchmarks complete$(RESET)\n"

## Count lines of Rust code
count:
	@printf "$(CYAN)$(BOLD)▶ Lines of Rust code:$(RESET)\n"
	@find . -name '*.rs' -not -path './target/*' | xargs wc -l | tail -1

# ==============================================================================
# Composite targets
# ==============================================================================

## Full CI pipeline: fmt-check + clippy + test + lint-arch
ci: fmt-check clippy test lint-arch
	@printf "$(GREEN)$(BOLD)✔ CI pipeline passed$(RESET)\n"

## Run fmt + lint + test (full CI check)
all: fmt lint test
	@printf "$(GREEN)$(BOLD)✔ All checks passed$(RESET)\n"

# ==============================================================================
# Help
# ==============================================================================

## Show this help
help:
	@printf "$(BOLD)KIAS - Makefile Commands$(RESET)\n"
	@printf "$(BOLD)========================$(RESET)\n"
	@printf "\n"
	@printf "$(BOLD)Build:$(RESET)\n"
	@printf "  $(GREEN)build$(RESET)              Build all crates (debug)\n"
	@printf "  $(GREEN)release$(RESET)            Build all crates (release)\n"
	@printf "\n"
	@printf "$(BOLD)Test:$(RESET)\n"
	@printf "  $(GREEN)test$(RESET)               Run all tests\n"
	@printf "  $(GREEN)test-verbose$(RESET)       Run tests with output\n"
	@printf "  $(GREEN)test-api$(RESET)           Run API server tests\n"
	@printf "  $(GREEN)test-scheduler$(RESET)     Run scheduler tests\n"
	@printf "  $(GREEN)test-controller$(RESET)    Run controller tests\n"
	@printf "\n"
	@printf "$(BOLD)Quality:$(RESET)\n"
	@printf "  $(GREEN)check$(RESET)              Run cargo check\n"
	@printf "  $(GREEN)clippy$(RESET)             Run clippy (warnings = errors)\n"
	@printf "  $(GREEN)fmt$(RESET)                Format code\n"
	@printf "  $(GREEN)fmt-check$(RESET)          Check formatting\n"
	@printf "  $(GREEN)lint$(RESET)               Run fmt-check + clippy\n"
	@printf "  $(GREEN)lint-arch$(RESET)          Check architecture layers (cargo metadata)\n"
	@printf "  $(GREEN)ci$(RESET)                Full CI pipeline (fmt + clippy + test + lint-arch)\n"
	@printf "  $(GREEN)all$(RESET)                Run fmt + lint + test\n"
	@printf "\n"
	@printf "$(BOLD)Run:$(RESET)\n"
	@printf "  $(GREEN)run-api$(RESET)            Run API server\n"
	@printf "  $(GREEN)run-monitor$(RESET)        Run monitor\n"
	@printf "\n"
	@printf "$(BOLD)Utility:$(RESET)\n"
	@printf "  $(GREEN)clean$(RESET)              Clean build artifacts\n"
	@printf "  $(GREEN)doc$(RESET)                Generate and open docs\n"
	@printf "  $(GREEN)bench$(RESET)             Run performance benchmarks\n"
	@printf "  $(GREEN)count$(RESET)              Count lines of Rust code\n"
	@printf "  $(GREEN)help$(RESET)               Show this help\n"
