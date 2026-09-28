#!/usr/bin/env bash
set -euo pipefail

export MENZI_TEST_DATABASE_URL="${MENZI_TEST_DATABASE_URL:-postgres://menzi:menzi@127.0.0.1:5432/menzi_test}"
export MENZI_TEST_NATS_URL="${MENZI_TEST_NATS_URL:-nats://127.0.0.1:4222}"

cargo test --workspace "$@"