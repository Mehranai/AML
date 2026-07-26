#!/usr/bin/env bash
set -euo pipefail

export PATH="${HOME}/.local/share/solana/install/active_release/bin:${PATH}"

CLUSTER="${SOLANA_CLUSTER:-mainnet-beta}"
IDENTITY_KEYPAIR="${SOLANA_IDENTITY_KEYPAIR:-/solana/config/validator-keypair.json}"
LEDGER_DIR="${SOLANA_LEDGER_DIR:-/solana/ledger}"
ACCOUNTS_DIR="${SOLANA_ACCOUNTS_DIR:-/solana/accounts}"
LOG_FILE="${SOLANA_LOG_FILE:-/solana/logs/agave-validator.log}"
RPC_BIND_ADDRESS="${SOLANA_RPC_BIND_ADDRESS:-0.0.0.0}"
RPC_PORT="${SOLANA_RPC_PORT:-8899}"
DYNAMIC_PORT_RANGE="${SOLANA_DYNAMIC_PORT_RANGE:-8000-8029}"

mkdir -p "$(dirname "${IDENTITY_KEYPAIR}")" "${LEDGER_DIR}" "${ACCOUNTS_DIR}" "$(dirname "${LOG_FILE}")"

if [ ! -f "${IDENTITY_KEYPAIR}" ]; then
    solana-keygen new --no-bip39-passphrase --silent --outfile "${IDENTITY_KEYPAIR}"
fi

ENTRYPOINT_ARGS=()
case "${CLUSTER}" in
    mainnet-beta)
        EXPECTED_GENESIS_HASH="${SOLANA_EXPECTED_GENESIS_HASH:-5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp}"
        ENTRYPOINT_ARGS=(
            --entrypoint entrypoint.mainnet-beta.solana.com:8001
            --entrypoint entrypoint2.mainnet-beta.solana.com:8001
            --entrypoint entrypoint3.mainnet-beta.solana.com:8001
            --entrypoint entrypoint4.mainnet-beta.solana.com:8001
            --entrypoint entrypoint5.mainnet-beta.solana.com:8001
        )
        ;;
    testnet)
        EXPECTED_GENESIS_HASH="${SOLANA_EXPECTED_GENESIS_HASH:-4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY}"
        ENTRYPOINT_ARGS=(
            --entrypoint entrypoint.testnet.solana.com:8001
            --entrypoint entrypoint2.testnet.solana.com:8001
            --entrypoint entrypoint3.testnet.solana.com:8001
        )
        ;;
    devnet)
        EXPECTED_GENESIS_HASH="${SOLANA_EXPECTED_GENESIS_HASH:-EtWTRABZaYq6iMfeYKouRu166VU2xqa1}"
        ENTRYPOINT_ARGS=(
            --entrypoint entrypoint.devnet.solana.com:8001
            --entrypoint entrypoint2.devnet.solana.com:8001
            --entrypoint entrypoint3.devnet.solana.com:8001
        )
        ;;
    *)
        echo "Unsupported SOLANA_CLUSTER=${CLUSTER}. Use mainnet-beta, testnet, or devnet." >&2
        exit 1
        ;;
esac

RPC_ARGS=()

if [ "${SOLANA_NO_VOTING:-true}" = "true" ]; then
    RPC_ARGS+=(--no-voting)
fi

if [ "${SOLANA_PRIVATE_RPC:-true}" = "true" ]; then
    RPC_ARGS+=(--private-rpc)
fi

if [ "${SOLANA_FULL_RPC_API:-true}" = "true" ]; then
    RPC_ARGS+=(--full-rpc-api)
fi

if [ "${SOLANA_ENABLE_RPC_TRANSACTION_HISTORY:-true}" = "true" ]; then
    RPC_ARGS+=(--enable-rpc-transaction-history)
fi

if [ "${SOLANA_ENABLE_CPI_AND_LOG_STORAGE:-true}" = "true" ]; then
    RPC_ARGS+=(--enable-cpi-and-log-storage)
fi

if [ "${SOLANA_LIMIT_LEDGER_SIZE:-false}" = "true" ]; then
    RPC_ARGS+=(--limit-ledger-size)
fi

exec agave-validator \
    --identity "${IDENTITY_KEYPAIR}" \
    --ledger "${LEDGER_DIR}" \
    --accounts "${ACCOUNTS_DIR}" \
    --log "${LOG_FILE}" \
    --rpc-bind-address "${RPC_BIND_ADDRESS}" \
    --rpc-port "${RPC_PORT}" \
    --dynamic-port-range "${DYNAMIC_PORT_RANGE}" \
    "${ENTRYPOINT_ARGS[@]}" \
    --expected-genesis-hash "${EXPECTED_GENESIS_HASH}" \
    --wal-recovery-mode skip_any_corrupted_record \
    "${RPC_ARGS[@]}" \
    "$@"
