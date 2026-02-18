#!/bin/bash
# Cipher NFT Staking - Kani Verification Runner

set -euo pipefail

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

collect_harnesses() {
    local files=(src/*_proofs.rs)
    [ -f "${files[0]}" ] || return 0
    grep -oE "fn prove_[a-z0-9_]+" "${files[@]}" 2>/dev/null | sed 's/fn //' | sort -u
}

proof_count() {
    collect_harnesses | wc -l | tr -d ' '
}

echo -e "${GREEN}================================================================${NC}"
echo -e "${GREEN}   Cipher NFT Staking - Formal Verification Suite${NC}"
echo -e "${GREEN}================================================================${NC}"
echo ""

# Check if Kani is installed
if ! command -v cargo-kani &> /dev/null; then
    echo -e "${RED}[ERROR] Kani is not installed!${NC}"
    echo ""
    echo "Install with:"
    echo "  cargo install --locked kani-verifier"
    echo "  cargo kani setup"
    exit 1
fi

echo -e "${GREEN}[OK] Kani verifier found: $(cargo kani --version)${NC}"
echo ""

# Parse arguments
if [ "${1:-}" == "--list" ]; then
    TOTAL=$(proof_count)
    if [ "$TOTAL" -eq 0 ]; then
        echo -e "${YELLOW}[WARN] No proof harnesses found yet${NC}"
        echo "Create proofs in src/*_proofs.rs files"
        exit 0
    fi
    echo "Available proof harnesses (${TOTAL} total):"
    echo ""
    for f in src/*_proofs.rs; do
        [ -f "$f" ] || continue
        module=$(basename "$f" .rs)
        count=$(grep -cE "fn prove_[a-z0-9_]+" "$f" 2>/dev/null || echo "0")
        echo "${module} (${count}):"
        grep -oE "fn prove_[a-z0-9_]+" "$f" 2>/dev/null | sed 's/fn /  - /' || true
        echo ""
    done
    echo "Run with: ./verify.sh --harness <name>"
    exit 0
fi

if [ "${1:-}" == "--harness" ]; then
    if [ -z "${2:-}" ]; then
        echo -e "${RED}[ERROR] Missing harness name${NC}"
        exit 1
    fi
    if ! collect_harnesses | grep -qx "$2"; then
        echo -e "${RED}[ERROR] Unknown harness: $2${NC}"
        exit 1
    fi
    echo -e "${YELLOW}[RUN] Running specific proof: $2${NC}"
    cargo kani --harness "$2" "${@:3}"
    exit 0
fi

if [ "${1:-}" == "--quick" ]; then
    TIMESTAMP=$(date +%Y%m%d_%H%M%S)
    PROOF_FILE="proofs/kani_quick_proof_${TIMESTAMP}.txt"

    mkdir -p proofs

    echo -e "${YELLOW}[RUN] Running quick verification (critical proofs only)${NC}"
    echo -e "${YELLOW}[INFO] Saving proof to: ${PROOF_FILE}${NC}"
    echo ""

    QUICK_HARNESSES=(
        prove_lock_enforcement
        prove_arithmetic_safety
        prove_owner_verification
    )

    # Filter to only harnesses that exist
    EXISTING_HARNESSES=()
    ALL_HARNESSES=$(collect_harnesses)
    for h in "${QUICK_HARNESSES[@]}"; do
        if echo "$ALL_HARNESSES" | grep -qx "$h"; then
            EXISTING_HARNESSES+=("$h")
        fi
    done

    if [ ${#EXISTING_HARNESSES[@]} -eq 0 ]; then
        echo -e "${YELLOW}[WARN] No quick harnesses found${NC}"
        exit 0
    fi

    ARGS=()
    for h in "${EXISTING_HARNESSES[@]}"; do
        ARGS+=(--harness "$h")
    done

    cargo kani "${ARGS[@]}" 2>&1 | tee "${PROOF_FILE}"

    RESULT=$?
    ACTUAL=$(grep -c "^Checking harness" "${PROOF_FILE}" 2>/dev/null || echo "0")
    EXPECTED=${#EXISTING_HARNESSES[@]}

    if [ $RESULT -eq 0 ]; then
        if [ "${ACTUAL}" != "${EXPECTED}" ]; then
            echo -e "${RED}[FAIL] Harness count mismatch! Expected ${EXPECTED}, got ${ACTUAL}${NC}"
            exit 1
        fi
        echo ""
        echo -e "${GREEN}[SUCCESS] Quick verification passed (${ACTUAL}/${EXPECTED} harnesses verified)${NC}"
        exit 0
    else
        echo -e "${RED}[FAIL] Quick verification failed${NC}"
        exit 1
    fi
fi

# Default: run all harnesses
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
PROOF_FILE="proofs/kani_full_proof_${TIMESTAMP}.txt"

mkdir -p proofs

TOTAL=$(proof_count)
if [ "$TOTAL" -eq 0 ]; then
    echo -e "${YELLOW}[WARN] No proof harnesses found${NC}"
    echo "Create proofs in src/*_proofs.rs files"
    exit 0
fi

echo -e "${YELLOW}[RUN] Running full verification (${TOTAL} harnesses)${NC}"
echo -e "${YELLOW}[INFO] This may take several minutes...${NC}"
echo -e "${YELLOW}[INFO] Saving proof to: ${PROOF_FILE}${NC}"
echo ""

cargo kani 2>&1 | tee "${PROOF_FILE}"

RESULT=$?
ACTUAL=$(grep -c "^Checking harness" "${PROOF_FILE}" 2>/dev/null || echo "0")

if [ $RESULT -eq 0 ]; then
    if [ "${ACTUAL}" != "${TOTAL}" ]; then
        echo -e "${RED}[FAIL] Harness count mismatch! Expected ${TOTAL}, got ${ACTUAL}${NC}"
        exit 1
    fi
    echo ""
    echo -e "${GREEN}[SUCCESS] Full verification passed (${ACTUAL}/${TOTAL} harnesses verified)${NC}"
    exit 0
else
    echo -e "${RED}[FAIL] Full verification failed${NC}"
    exit 1
fi
