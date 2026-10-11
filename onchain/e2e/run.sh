#!/usr/bin/env bash
# basket_vault end-to-end on a local validator that clones Panta's mainnet
# program and two live markets. Nothing touches mainnet state.
#
#   ./onchain/e2e/run.sh            (after `anchor build` in onchain/)
#
# Needs: solana-test-validator on PATH; mainnet RPC reachable for cloning.
# Both legs must be markets still in Panta's primary (flash acquisition)
# window — pass them as "event market_authority market_vault":
#   E2E_MARKET1="..." E2E_MARKET2="..." ./onchain/e2e/run.sh
set -euo pipefail
: "${E2E_MARKET1:=CLQqZ7r6WYC5xNumWrmC7UAkQ1Z6pdCbjJSy7wQu1DgL 6yqZocntGBaqicfmdHCE4NLLYtSyHDwgub5ViGQcz2Sz GtAbAzTVMzyFanTT2wY9atDeWL2L61K4YnJNF9jX6ZVq}"
: "${E2E_MARKET2:?set E2E_MARKET2 to a second market in its primary window}"
export E2E_MARKET1 E2E_MARKET2
CLONES=()
for addr in $E2E_MARKET1 $E2E_MARKET2; do CLONES+=(--clone "$addr"); done

HERE="$(cd "$(dirname "$0")" && pwd)"
ONCHAIN="$(dirname "$HERE")"
TARGET="${CARGO_TARGET_DIR:-$HERE/target}"
WORK="$(mktemp -d)"
trap 'kill $VALIDATOR 2>/dev/null || true; rm -rf "$WORK"' EXIT

(cd "$HERE" && CARGO_TARGET_DIR="$TARGET" cargo build -q)
E2E="$TARGET/debug/vault_e2e"

read -r ADMIN USER < <("$E2E" fixtures "$WORK")
USER_USDC=$(solana find-program-derived-address ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL \
  "pubkey:$USER" pubkey:TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA \
  pubkey:EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v | head -1)

solana-test-validator --ledger "$WORK/ledger" --reset --quiet \
  --url https://api.mainnet-beta.solana.com \
  --clone-upgradeable-program 6gM5afTQBq5VZCfgpGqcsqzfWd5maLSCKWtGjbEobZMp \
  --clone EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v \
  --clone 8mJjfx7SuWwS4yfVZXtqC2cvzxr3TQHsCgKyjpuDWKAP \
  --clone 9hwZLD2JZuWFDDQXEnHsnny7EF2LeyHqwcMFVssYEGpC \
  "${CLONES[@]}" \
  --account "$USER_USDC" "$WORK/user_usdc.json" \
  --bpf-program AGrL8gCofvFTvsLDXmwThAWd1Wdz9ULPYwHWCzVDnF7u "$ONCHAIN/target/deploy/basket_vault.so" \
  --mint "$ADMIN" > "$WORK/validator.log" 2>&1 &
VALIDATOR=$!

for _ in $(seq 1 60); do
  solana -u localhost cluster-version >/dev/null 2>&1 && break
  kill -0 $VALIDATOR 2>/dev/null || { cat "$WORK/validator.log"; exit 1; }
  sleep 2
done

"$E2E" run
