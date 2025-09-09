.PHONY: help install dev lint format format-check test test-verbose build clean all check

# Default target
.DEFAULT_GOAL := help

# Variables
PYTHON := python3
UV := uv
PACKAGE_NAME := oasis-rofl-client-py
SRC_DIR := src
TEST_DIR := tests
EXAMPLE_DIR := examples

# Colors for output
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[0;33m
RED := \033[0;31m
NC := \033[0m # No Color

help: ## Show this help message
	@echo "$(BLUE)Available targets:$(NC)"
	@echo ""
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  $(GREEN)%-15s$(NC) %s\n", $$1, $$2}'
	@echo ""
	@echo "$(YELLOW)Usage:$(NC) make [target]"

# Development Setup
install: ## Install package in editable mode with dev dependencies
	@echo "$(BLUE)Installing package with dev dependencies...$(NC)"
	$(UV) sync --dev

dev: install ## Setup development environment
	@echo "$(GREEN)Development environment ready!$(NC)"

# Code Quality
lint: ## Run ruff linter
	@echo "$(BLUE)Running ruff linter...$(NC)"
	$(UV) run ruff check $(SRC_DIR) $(TEST_DIR) $(EXAMPLE_DIR)

format: ## Format code with ruff
	@echo "$(BLUE)Formatting code with ruff...$(NC)"
	$(UV) run ruff format $(SRC_DIR) $(TEST_DIR) $(EXAMPLE_DIR)
	$(UV) run ruff check --fix $(SRC_DIR) $(TEST_DIR) $(EXAMPLE_DIR)

format-check: ## Check code formatting without modifying files
	@echo "$(BLUE)Checking code format...$(NC)"
	$(UV) run ruff format --check $(SRC_DIR) $(TEST_DIR) $(EXAMPLE_DIR)
	$(UV) run ruff check $(SRC_DIR) $(TEST_DIR) $(EXAMPLE_DIR)

# Testing
test: ## Run tests with pytest
	@echo "$(BLUE)Running tests...$(NC)"
	$(UV) run pytest $(TEST_DIR)

test-verbose: ## Run tests with verbose output
	@echo "$(BLUE)Running tests (verbose)...$(NC)"
	$(UV) run pytest -vv $(TEST_DIR)

test-coverage: ## Run tests with coverage report (requires pytest-cov)
	@echo "$(BLUE)Running tests with coverage...$(NC)"
	@if $(UV) run pip show pytest-cov > /dev/null 2>&1; then \
		$(UV) run pytest --cov=$(PACKAGE_NAME) --cov-report=term-missing $(TEST_DIR); \
	else \
		echo "$(YELLOW)Warning: pytest-cov not installed. Run 'uv add --dev pytest-cov' to enable coverage reports.$(NC)"; \
		$(UV) run pytest $(TEST_DIR); \
	fi

# Build
build: clean ## Build distribution packages
	@echo "$(BLUE)Building distribution packages...$(NC)"
	$(UV) build

clean: ## Clean build artifacts and caches
	@echo "$(BLUE)Cleaning build artifacts...$(NC)"
	rm -rf build/
	rm -rf dist/
	rm -rf *.egg-info
	rm -rf src/*.egg-info
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name .pytest_cache -exec rm -rf {} + 2>/dev/null || true
	find . -type d -name .ruff_cache -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name "*.pyc" -delete
	find . -type f -name "*.pyo" -delete
	find . -type f -name "*~" -delete
	@echo "$(GREEN)Clean complete!$(NC)"

# Combined targets
all: format lint test ## Run format, lint, and test

check: format-check lint test ## Run all checks without modifying files

# Quick checks
quick: ## Run quick lint and test (no formatting)
	@echo "$(BLUE)Running quick checks...$(NC)"
	@$(MAKE) lint
	@$(MAKE) test

# Type checking (if mypy is added later)
typecheck: ## Run type checking with mypy (if installed)
	@if $(UV) run pip show mypy > /dev/null 2>&1; then \
		echo "$(BLUE)Running type checks...$(NC)"; \
		$(UV) run mypy $(SRC_DIR); \
	else \
		echo "$(YELLOW)mypy not installed. Run 'uv add --dev mypy' to enable type checking.$(NC)"; \
	fi