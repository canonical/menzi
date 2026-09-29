#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
  source "$HOME/.cargo/env"
fi

"$ROOT/infra/local/setup.sh"

DEV_HOST="${MENZI_DEV_HOST:-127.0.0.1}"
OPEN=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --open) OPEN=1 ;;
    --host=*) DEV_HOST="${1#--host=}" ;;
    --host) shift; DEV_HOST="${1:-$DEV_HOST}" ;;
    *) printf 'usage: %s [--open] [--host HOST]\n' "${0##*/}" >&2; exit 2 ;;
  esac
  shift
done

WAIT_HOST="$DEV_HOST"
if [ "$DEV_HOST" = "0.0.0.0" ]; then
  WAIT_HOST="127.0.0.1"
fi

DISPLAY_HOST="$DEV_HOST"
if [ "$DEV_HOST" = "0.0.0.0" ] || [ "$DEV_HOST" = "::" ]; then
  LAN_IP="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="src"){print $(i+1); exit}}')"
  if [ -z "$LAN_IP" ]; then
    LAN_IP="$(hostname -I 2>/dev/null | awk '{print $1}')"
  fi
  DISPLAY_HOST="${LAN_IP:-$WAIT_HOST}"
fi

API_BIND="${MENZI_API_BIND:-$DEV_HOST:8080}"
ORCH_BIND="${MENZI_ORCHESTRATOR_BIND:-$DEV_HOST:8081}"
PREVIEWS_BIND="${MENZI_PREVIEWS_BIND:-$DEV_HOST:8095}"
PROXY_BIND="${MENZI_SESSION_PROXY_BIND:-$DEV_HOST:8082}"
LLM_BIND="${MENZI_LLM_GATEWAY_BIND:-$DEV_HOST:8083}"
OPENCODE_PORT="${MENZI_OPENCODE_PORT:-17999}"
STUB_MODEL_PORT="${MENZI_STUB_MODEL_PORT:-18000}"
VITE_PORT="${MENZI_VITE_PORT:-5173}"
DB_URL="${MENZI_DATABASE_URL:-postgres://menzi:menzi@127.0.0.1:5432/menzi}"

LOG_DIR="$ROOT/target/dev"
mkdir -p "$LOG_DIR"

if [ ! -d "$ROOT/frontend/node_modules" ]; then
  (cd "$ROOT/frontend" && npm install)
fi

cargo build -p menzi-core-api -p menzi-orchestrator -p menzi-previews -p menzi-session-proxy -p menzi-llm-gateway

if [ -n "${MENZI_LXD_CERT_PATH:-}" ] || [ -n "${MENZI_LXD_KEY_PATH:-}" ]; then
  :
elif [ -f "$HOME/.config/lxc/client.crt" ] && [ -f "$HOME/.config/lxc/client.key" ]; then
  export MENZI_LXD_CERT_PATH="$HOME/.config/lxc/client.crt"
  export MENZI_LXD_KEY_PATH="$HOME/.config/lxc/client.key"
fi

PIDS=()

cleanup() {
  for pid in "${PIDS[@]:-}"; do
    kill -TERM -- "-$pid" 2>/dev/null || true
    sleep 0.2
    kill -KILL -- "-$pid" 2>/dev/null || true
  done
}
trap cleanup EXIT INT TERM

setsid env MENZI_DATABASE_URL="$DB_URL" "$ROOT/target/debug/menzi-core-api" >"$LOG_DIR/core-api.log" 2>&1 &
PIDS+=("$!")
setsid env MENZI_GATEWAY_BIND="$ORCH_BIND" "$ROOT/target/debug/menzi-orchestrator" >"$LOG_DIR/orchestrator.log" 2>&1 &
PIDS+=("$!")
setsid env MENZI_PREVIEWS_BIND="$PREVIEWS_BIND" "$ROOT/target/debug/menzi-previews" >"$LOG_DIR/previews.log" 2>&1 &
PIDS+=("$!")
setsid env MENZI_GATEWAY_BIND="$PROXY_BIND" "$ROOT/target/debug/menzi-session-proxy" >"$LOG_DIR/session-proxy.log" 2>&1 &
PIDS+=("$!")
setsid env MENZI_GATEWAY_BIND="$LLM_BIND" "$ROOT/target/debug/menzi-llm-gateway" >"$LOG_DIR/llm-gateway.log" 2>&1 &
PIDS+=("$!")

wait_http() {
  local url="$1" name="$2" tries="${3:-60}" want="${4:-}" code
  for _ in $(seq 1 "$tries"); do
    code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 2 "$url" || true)"
    if [ -n "$want" ]; then
      if [ "$code" = "$want" ]; then
        echo "$name up ($code) at $url"
        return 0
      fi
    elif [ "$code" != "000" ]; then
      echo "$name up ($code) at $url"
      return 0
    fi
    sleep 1
  done
  if [ -n "$want" ]; then
    echo "error: $name did not answer $want at $url; see $LOG_DIR" >&2
  else
    echo "error: $name did not come up at $url; see $LOG_DIR" >&2
  fi
  return 1
}

if [ "${MENZI_SKIP_OPENCODE:-0}" != "1" ]; then
  if [ "${MENZI_STUB_MODEL:-0}" = "1" ]; then
    setsid env MENZI_STUB_MODEL_PORT="$STUB_MODEL_PORT" node "$ROOT/infra/local/stub-model.mjs" \
      >"$LOG_DIR/stub-model.log" 2>&1 &
    PIDS+=("$!")
    wait_http "http://127.0.0.1:$STUB_MODEL_PORT/v1/models" "stub model" 30 200
  fi
  if [ "${MENZI_OPENCODE_HOST_OPENCODE:-0}" = "1" ] && command -v opencode >/dev/null 2>&1; then
    if [ "${MENZI_OPENCODE_CONFIG:-overwrite}" = "overwrite" ] || [ ! -f "$HOME/.config/opencode/opencode.json" ]; then
      mkdir -p "$HOME/.config/opencode"
      cp "$ROOT/infra/local/opencode.json" "$HOME/.config/opencode/opencode.json"
    fi
    setsid env MENZI_OPENCODE_CONFIG_DIR="$HOME/.config/opencode" \
      opencode serve --port "$OPENCODE_PORT" --hostname 127.0.0.1 \
      >"$LOG_DIR/opencode.log" 2>&1 &
    PIDS+=("$!")
    OPENCODE_URL="http://127.0.0.1:$OPENCODE_PORT"
    wait_http "$OPENCODE_URL/api/model" "opencode" 60 200
  else
    OPENCODE_URL="${MENZI_OPENCODE_URL:-http://10.10.10.251:4096}"
    wait_http "$OPENCODE_URL/api/model" "opencode in lxd" 60 200
  fi
fi

wait_http "http://${API_BIND/0.0.0.0/$WAIT_HOST}/health" "control plane" 60 200
wait_http "http://${ORCH_BIND/0.0.0.0/$WAIT_HOST}/api/env/health" "orchestrator" 60 200
wait_http "http://${PREVIEWS_BIND/0.0.0.0/$WAIT_HOST}/api/v1/previews" "previews" 60 405
wait_http "http://${PROXY_BIND/0.0.0.0/$WAIT_HOST}/health" "session proxy" 60 200
wait_http "http://${LLM_BIND/0.0.0.0/$WAIT_HOST}/v1/models" "llm gateway" 60 200

if [ -n "${OPENCODE_URL:-}" ]; then
  if curl -s -f -X POST "http://${PROXY_BIND/0.0.0.0/$WAIT_HOST}/api/opencode/register" \
    -H 'content-type: application/json' --max-time 5 \
    -d "{\"url\":\"$OPENCODE_URL\"}" >/dev/null 2>&1; then
    echo "opencode registered with session proxy"
  else
    echo "warning: opencode registration failed; the proxy keeps its default target" >&2
  fi
fi

if [ "${MENZI_SKIP_SEED:-0}" != "1" ] && command -v psql >/dev/null 2>&1; then
  PGPASSWORD="menzi" psql -q -h 127.0.0.1 -U menzi -d menzi -v ON_ERROR_STOP=1 \
    -f "$ROOT/infra/local/seed.sql" >"$LOG_DIR/seed.log" 2>&1 \
    && echo "seed data applied" \
    || echo "warning: seed failed, see $LOG_DIR/seed.log" >&2
fi

setsid env -C "$ROOT/frontend" npm run dev -- --host "$DEV_HOST" --port "$VITE_PORT" >"$LOG_DIR/vite.log" 2>&1 &
VITE_PID=$!
PIDS+=("$VITE_PID")
wait_http "http://$WAIT_HOST:$VITE_PORT/" "frontend" 60 200

printf '\nmenzi dev stack running\n'
printf '  control plane   http://%s\n' "${API_BIND/0.0.0.0/$DISPLAY_HOST}"
printf '  orchestrator    http://%s\n' "${ORCH_BIND/0.0.0.0/$DISPLAY_HOST}"
printf '  previews        http://%s\n' "${PREVIEWS_BIND/0.0.0.0/$DISPLAY_HOST}"
printf '  session proxy   http://%s\n' "${PROXY_BIND/0.0.0.0/$DISPLAY_HOST}"
printf '  llm gateway     http://%s\n' "${LLM_BIND/0.0.0.0/$DISPLAY_HOST}"
printf '  opencode        %s\n' "${OPENCODE_URL:-skipped}"
printf '  frontend        http://%s:%s\n' "$DISPLAY_HOST" "$VITE_PORT"
printf '  logs            %s\n' "$LOG_DIR"

if [ "$OPEN" = "1" ]; then
  xdg-open "http://$WAIT_HOST:$VITE_PORT" 2>/dev/null || true
fi

wait "$VITE_PID"