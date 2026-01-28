# Noir Circuits - Build, Test, and Demo Commands
# Run `just --list` to see all available commands

# Default: list available commands
default:
    @just --list

# Ensure sunspot is in PATH
export PATH := env_var("HOME") + "/sunspot/go:" + env_var("PATH")
export GNARK_VERIFIER_BIN := env_var("HOME") + "/sunspot/gnark-solana/crates/verifier-bin"

# ============================================================================
# Quick Start (run in order for first-time setup)
# ============================================================================
# 1. just install-all       - Install dependencies
# 2. just test-all          - Run circuit unit tests
# 3. just verify-all        - Verify proofs on-chain (uses pre-deployed verifiers)

# Install all dependencies (run this first!)
install-all: install-lib install-claim install-finalize

# Install shared lib dependencies
install-lib:
    cd lib && pnpm install

# Test all circuits (nargo test)
test-all: test-claim test-finalize

# Compile all circuits
compile-all: compile-claim compile-finalize

# Generate proofs for all circuits (uses existing keys)
prove-all: prove-claim prove-finalize

# Verify all proofs on-chain (requires deployed verifiers)
verify-all: verify-claim verify-finalize

# ============================================================================
# circuits/claim
# ============================================================================

# Install client dependencies
install-claim:
    cd circuits/claim/client && pnpm install

# Compile circuit
compile-claim:
    cd circuits/claim && nargo compile

# Run circuit tests
test-claim:
    cd circuits/claim && nargo test

# Generate witness
execute-claim:
    cd circuits/claim && nargo execute

# Generate proof (uses existing ACIR/pk/ccs from repo)
prove-claim: execute-claim
    cd circuits/claim && sunspot prove target/claim.json target/claim.gz target/claim.ccs target/claim.pk

# Verify proof on-chain (requires deployed verifier program)
verify-claim:
    cd circuits/claim/client && pnpm run verify
    git checkout circuits/claim/Prover.toml 2>/dev/null || true

# Full Sunspot setup (regenerates keys - only needed if circuit changes)
setup-claim: compile-claim execute-claim
    cd circuits/claim && sunspot compile target/claim.json
    cd circuits/claim && sunspot setup target/claim.ccs
    cd circuits/claim && sunspot prove target/claim.json target/claim.gz target/claim.ccs target/claim.pk

# Build Solana verifier program
build-verifier-claim:
    cd circuits/claim && sunspot deploy target/claim.vk

# ============================================================================
# circuits/finalize
# ============================================================================

# Install client dependencies (if they exist)
install-finalize:
    # cd circuits/finalize/client && pnpm install
    @echo "Client installation for finalize skipped (directory may not exist yet)"

# Compile circuit
compile-finalize:
    cd circuits/finalize && nargo compile

# Run circuit tests
test-finalize:
    cd circuits/finalize && nargo test

# Generate witness
execute-finalize:
    cd circuits/finalize && nargo execute

# Generate proof (uses existing ACIR/pk/ccs from repo)
prove-finalize: execute-finalize
    cd circuits/finalize && sunspot prove target/finalize.json target/finalize.gz target/finalize.ccs target/finalize.pk

# Verify proof on-chain (requires deployed verifier program)
# Note: Requires client structure to be set up.
verify-finalize:
    # cd circuits/finalize/client && pnpm run verify
    @echo "Verification for finalize skipped (client not set up yet)"

# Full Sunspot setup (regenerates keys - only needed if circuit changes)
setup-finalize: compile-finalize execute-finalize
    cd circuits/finalize && sunspot compile target/finalize.json
    cd circuits/finalize && sunspot setup target/finalize.ccs
    cd circuits/finalize && sunspot prove target/finalize.json target/finalize.gz target/finalize.ccs target/finalize.pk

# Build Solana verifier program
build-verifier-finalize:
    cd circuits/finalize && sunspot deploy target/finalize.vk

# ============================================================================
# Utility Commands
# ============================================================================

# Format all code (Noir + Rust + TypeScript)
fmt:
    cd circuits/claim && nargo fmt
    cd circuits/finalize && nargo fmt
    cd lib && npx prettier --write "../**/*.ts"

# Check formatting
fmt-check:
    cd circuits/claim && nargo fmt --check
    cd circuits/finalize && nargo fmt --check
    cd lib && npx prettier --check "../**/*.ts"

# Check nargo/sunspot versions
version:
    nargo --version
    sunspot --version || echo "sunspot not installed"