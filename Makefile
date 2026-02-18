# ===========================================
# Makefile - Cipher NFT Staking
# ===========================================

SHELL                := /usr/bin/env bash
.SHELLFLAGS          := -euo pipefail -c
.ONESHELL:
MAKEFLAGS           += --no-builtin-rules
.SILENT:

# Tooling
ANCHOR               ?= anchor
SOLANA               ?= solana
CARGO                ?= cargo
VALIDATOR            ?= solana-test-validator

# Project
PROGRAM_NAME         ?= cipher_nft_staking
PROGRAM_DIR          ?= programs/$(PROGRAM_NAME)
PROGRAM_KEYPAIR      ?= target/deploy/$(PROGRAM_NAME)-keypair.json
IDL_JSON             ?= target/idl/$(PROGRAM_NAME).json

# Wallet
DEPLOY_WALLET        ?= ~/.config/solana/id.json

# Keys directory
KEYS_DIR             ?= programs/$(PROGRAM_NAME)/keys
CANONICAL_KEY        ?= $(KEYS_DIR)/$(PROGRAM_NAME)-keypair.json

# Local validator config
RPC_HOST             ?= 127.0.0.1
RPC_PORT             ?= 8899
RPC_URL              ?= http://$(RPC_HOST):$(RPC_PORT)
LEDGER_DIR           ?= .ledger
PIDFILE              ?= .local-validator.pid
VALIDATOR_LOG        ?= .validator.log

# Cluster
CLUSTER              ?= $(shell awk -F'= ' '/^cluster/ {print $$2}' Anchor.toml 2>/dev/null | tr -d '"')
CLUSTER              ?= Localnet

# Mainnet config
MAINNET_URL          ?= https://api.mainnet-beta.solana.com
PROGRAM_ID           ?= CiPherNFTStake11111111111111111111111111111

# Optional .env
ifneq (,$(wildcard .env))
	include .env
	export
endif

# Colors
C_RESET  := \033[0m
C_INFO   := \033[1;34m
C_WARN   := \033[1;33m
C_OK     := \033[1;32m
C_ERR    := \033[1;31m

.PHONY: help
help:
	printf "$(C_INFO)Cipher NFT Staking — available commands$(C_RESET)\n"
	awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z0-9_\-]+:.*##/ {printf "  \033[1;36m%-22s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

.PHONY: doctor
doctor: ## Show versions & current config
	echo -e "$(C_INFO)=> Versions$(C_RESET)"
	$(ANCHOR) --version || true
	$(SOLANA) --version || true
	$(CARGO) --version || true
	echo -e "$(C_INFO)=> Anchor cluster:$(C_RESET) $(CLUSTER)"
	echo -e "$(C_INFO)=> Program:$(C_RESET) $(PROGRAM_NAME)"
	echo -e "$(C_INFO)=> Deploy wallet:$(C_RESET) $(DEPLOY_WALLET)"
	[ -f Anchor.toml ] || (echo -e "$(C_ERR)Missing Anchor.toml$(C_RESET)"; exit 1)

# ===== Key management =====
.PHONY: ensure-key
ensure-key:
	mkdir -p target/deploy
	if [ ! -f "$(PROGRAM_KEYPAIR)" ]; then \
	  if [ -f "$(CANONICAL_KEY)" ]; then \
	    echo "Restoring $(PROGRAM_KEYPAIR) from $(CANONICAL_KEY)"; \
	    cp "$(CANONICAL_KEY)" "$(PROGRAM_KEYPAIR)"; \
	  else \
	    echo -e "$(C_WARN)No canonical key at $(CANONICAL_KEY). Anchor will generate one.$(C_RESET)"; \
	  fi; \
	fi

# ===== Build / Lint / Format =====
.PHONY: build
build: ensure-key ## Build the program (debug)
	echo -e "$(C_INFO)=> Building$(C_RESET)"
	$(ANCHOR) build

.PHONY: build-release
build-release: ensure-key ## Build the program (optimized release)
	echo -e "$(C_INFO)=> Building (release)$(C_RESET)"
	RUSTFLAGS="-C target-cpu=native" $(ANCHOR) build -- --release

.PHONY: lint
lint: ## Run clippy lints
	echo -e "$(C_INFO)=> Clippy$(C_RESET)"
	$(CARGO) clippy --all -- -D warnings

.PHONY: fmt
fmt: ## Format Rust sources
	echo -e "$(C_INFO)=> Formatting$(C_RESET)"
	$(CARGO) fmt

.PHONY: fmt-check
fmt-check: ## Check if code is formatted
	echo -e "$(C_INFO)=> Checking format$(C_RESET)"
	$(CARGO) fmt -- --check

# ===== Validator control =====
.PHONY: kill-validator
kill-validator: ## Kill any running validator
	echo -e "$(C_INFO)=> Killing existing validator$(C_RESET)"
	pkill -f "$(VALIDATOR)" >/dev/null 2>&1 || true
	if lsof -ti :$(RPC_PORT) >/dev/null 2>&1; then \
		echo -e "$(C_WARN)Port $(RPC_PORT) in use; freeing...$(C_RESET)"; \
		kill -9 $$(lsof -ti :$(RPC_PORT)) || true; \
	fi
	rm -f "$(PIDFILE)" "$(VALIDATOR_LOG)" || true
	mkdir -p "$(LEDGER_DIR)"
	echo -e "$(C_OK)Clean slate ready$(C_RESET)"

.PHONY: start-validator
start-validator: kill-validator ## Start local validator (fresh)
	echo -e "$(C_INFO)=> Starting local validator on $(RPC_URL)$(C_RESET)"
	$(VALIDATOR) \
		--reset \
		--rpc-port $(RPC_PORT) \
		--ledger $(LEDGER_DIR) \
		--limit-ledger-size \
		> "$(VALIDATOR_LOG)" 2>&1 & \
	echo $$! > "$(PIDFILE)"
	$(MAKE) wait-rpc || { echo -e "$(C_ERR)Validator failed to start$(C_RESET)"; $(MAKE) tail-logs; exit 1; }

.PHONY: wait-rpc
wait-rpc:
	echo -e "$(C_INFO)=> Waiting for RPC $(RPC_URL)$(C_RESET)"
	i=0; max=300; \
	while ! $(SOLANA) -u $(RPC_URL) cluster-version >/dev/null 2>&1; do \
		if [ -f "$(PIDFILE)" ]; then \
			PID=$$(cat "$(PIDFILE)"); \
			if ! kill -0 $$PID 2>/dev/null; then \
				echo -e "$(C_ERR)Validator process exited$(C_RESET)"; \
				exit 1; \
			fi; \
		fi; \
		i=$$((i+1)); \
		if [ $$i -ge $$max ]; then \
			echo -e "$(C_ERR)RPC did not come up in time$(C_RESET)"; \
			exit 1; \
		fi; \
		sleep 1; \
	done; \
	echo -e "$(C_OK)RPC is up$(C_RESET)"

.PHONY: stop-validator
stop-validator: ## Stop local validator
	if [ -f "$(PIDFILE)" ]; then \
		PID=$$(cat "$(PIDFILE)"); \
		if kill -0 $$PID 2>/dev/null; then \
			echo -e "$(C_INFO)=> Stopping validator (PID $$PID)$(C_RESET)"; \
			kill $$PID; \
			wait $$PID 2>/dev/null || true; \
		fi; \
		rm -f "$(PIDFILE)"; \
	fi

.PHONY: tail-logs
tail-logs:
	echo "----- $(VALIDATOR_LOG) (last 100 lines) -----"
	tail -n 100 "$(VALIDATOR_LOG)" || true
	echo "---------------------------------------------"

# ===== Deploy / IDL =====
.PHONY: deploy
deploy: build ## Deploy to cluster in Anchor.toml
	echo -e "$(C_INFO)=> Deploying to $(CLUSTER)$(C_RESET)"
	$(ANCHOR) deploy

.PHONY: deploy-local
deploy-local: ensure-key build start-validator ## Build & deploy to local validator
	echo -e "$(C_INFO)=> Configuring Solana CLI$(C_RESET)"
	$(SOLANA) config set --url $(RPC_URL) >/dev/null
	$(SOLANA) config set --keypair $(DEPLOY_WALLET) >/dev/null
	echo -e "$(C_INFO)=> Airdropping SOL$(C_RESET)"
	$(SOLANA) -u $(RPC_URL) airdrop 20 || true
	$(SOLANA) -u $(RPC_URL) balance || true
	echo -e "$(C_INFO)=> Deploying program$(C_RESET)"
	ANCHOR_PROVIDER_URL=$(RPC_URL) ANCHOR_WALLET=$(DEPLOY_WALLET) $(ANCHOR) deploy

.PHONY: idl
idl: build ## Export IDL json
	echo -e "$(C_INFO)=> Exporting IDL$(C_RESET)"
	[ -f "$(IDL_JSON)" ] && echo "IDL at $(IDL_JSON)" || echo "IDL generated by anchor build"

.PHONY: idl-show
idl-show: ## Print current IDL
	[ -f "$(IDL_JSON)" ] || (echo -e "$(C_WARN)IDL not found; run 'make build'$(C_RESET)"; exit 1)
	jq . "$(IDL_JSON)" | less

# ===== Tests =====
.PHONY: test
test: ## Fresh validator -> build -> deploy -> tests
	set -e
	trap '$(MAKE) stop-validator' EXIT
	$(MAKE) start-validator
	$(SOLANA) config set --url $(RPC_URL) >/dev/null
	$(SOLANA) config set --keypair $(DEPLOY_WALLET) >/dev/null
	echo -e "$(C_INFO)=> Airdropping SOL$(C_RESET)"
	$(SOLANA) -u $(RPC_URL) airdrop 20 || true
	$(SOLANA) -u $(RPC_URL) balance || true
	export ANCHOR_PROVIDER_URL=$(RPC_URL); \
	export ANCHOR_WALLET=$(DEPLOY_WALLET); \
	export ANCHOR_SKIP_LOCAL_VALIDATOR=1; \
	echo -e "$(C_INFO)=> Building$(C_RESET)"; \
	$(MAKE) ensure-key; \
	$(ANCHOR) build; \
	echo -e "$(C_INFO)=> Deploying$(C_RESET)"; \
	$(ANCHOR) deploy; \
	echo -e "$(C_INFO)=> Running tests$(C_RESET)"; \
	$(ANCHOR) run test

.PHONY: test-fast
test-fast: ## Run tests against running validator (no rebuild)
	$(SOLANA) config set --url $(RPC_URL) >/dev/null
	$(SOLANA) config set --keypair $(DEPLOY_WALLET) >/dev/null
	export ANCHOR_PROVIDER_URL=$(RPC_URL); \
	export ANCHOR_WALLET=$(DEPLOY_WALLET); \
	export ANCHOR_SKIP_LOCAL_VALIDATOR=1; \
	echo -e "$(C_INFO)=> Running tests (fast)$(C_RESET)"; \
	$(ANCHOR) run test

# ===== Verification =====
.PHONY: verify
verify: ## Run Kani verification (quick)
	echo -e "$(C_INFO)=> Running Kani verification (quick)$(C_RESET)"
	cd kani_verification && ./verify.sh --quick

.PHONY: verify-all
verify-all: ## Run all Kani proofs (slow)
	echo -e "$(C_INFO)=> Running full Kani verification$(C_RESET)"
	cd kani_verification && ./verify.sh

.PHONY: verify-list
verify-list: ## List all available Kani proofs
	cd kani_verification && ./verify.sh --list

# ===== Convenience =====
.PHONY: status
status: ## Show Solana CLI config
	$(SOLANA) config get

.PHONY: airdrop
airdrop: ## Airdrop SOL (default 5). Usage: make airdrop AMOUNT=10
	AMOUNT=${AMOUNT:-5}; \
	echo -e "$(C_INFO)=> Airdrop $$AMOUNT SOL$(C_RESET)"; \
	$(SOLANA) -u $(RPC_URL) airdrop $$AMOUNT || true

.PHONY: accounts
accounts: ## List program accounts
	echo -e "$(C_INFO)=> Program accounts$(C_RESET)"
	$(SOLANA) program show --programs || true

# ===== Cleanup =====
.PHONY: clean
clean: ## Clean cargo artifacts (keeps program key)
	echo -e "$(C_INFO)=> Cleaning$(C_RESET)"
	$(CARGO) clean

.PHONY: clean-all
clean-all: ## Deep clean (keeps canonical key)
	$(MAKE) clean
	rm -rf .anchor node_modules .cache $(LEDGER_DIR) $(PIDFILE) $(VALIDATOR_LOG) 2>/dev/null || true
	echo -e "$(C_OK)Done$(C_RESET)"

.PHONY: check-program-id
check-program-id:
	@echo -e "$(C_INFO)=> Program ID (pinned):$(C_RESET) $(PROGRAM_ID)"
	@test -f "$(PROGRAM_KEYPAIR)" || (echo -e "$(C_ERR)Missing $(PROGRAM_KEYPAIR)$(C_RESET)"; exit 1)
	@KEYPAIR_PUB=$$($(SOLANA) address -k "$(PROGRAM_KEYPAIR)"); \
	if [ "$$KEYPAIR_PUB" != "$(PROGRAM_ID)" ]; then \
	  echo -e "$(C_ERR)Program keypair mismatch!$(C_RESET)"; \
	  echo -e "$(C_ERR)Found: $$KEYPAIR_PUB$(C_RESET)"; \
	  echo -e "$(C_ERR)Expected: $(PROGRAM_ID)$(C_RESET)"; \
	  exit 1; \
	fi
	@echo -e "$(C_OK)Program keypair matches$(C_RESET)"
