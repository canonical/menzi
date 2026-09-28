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
VITE_PORT="${MENZI_VITE_PORT:-5173}"
DB_URL="${MENZI_DATABASE_URL:-postgres://menzi:menzi@127.0.0.1:5432/menzi}"

LOG_DIR="$ROOT/target/dev"
mkdir -p "$LOG_DIR"

if [ ! -d "$ROOT/frontend/node_modules" ]; then
  (cd "$ROOT/frontend" && npm install)
fi

cargo build -p menzi-core-api -p menzi-orchestrator -p menzi-previews

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

wait_http() {
  local url="$1" name="$2" tries="${3:-60}" code
  for _ in $(seq 1 "$tries"); do
    code="$(curl -s -o /dev/null -w '%{http_code}' --max-time 2 "$url" || true)"
    if [ "$code" != "000" ]; then
      echo "$name up ($code) at $url"
      return 0
    fi
    sleep 1
  done
  echo "error: $name did not come up at $url; see $LOG_DIR" >&2
  return 1
}

wait_http "http://${API_BIND/0.0.0.0/$WAIT_HOST}/health" "control plane"
wait_http "http://${ORCH_BIND/0.0.0.0/$WAIT_HOST}/api/env/status" "orchestrator"
wait_http "http://${PREVIEWS_BIND/0.0.0.0/$WAIT_HOST}/api/v1/previews" "previews"

setsid env -C "$ROOT/frontend" npm run dev -- --host "$DEV_HOST" --port "$VITE_PORT" >"$LOG_DIR/vite.log" 2>&1 &
VITE_PID=$!
PIDS+=("$VITE_PID")
wait_http "http://$WAIT_HOST:$VITE_PORT/" "frontend"

printf '\nmenzi dev stack running\n'
printf '  control plane  http://%s\n' "${API_BIND/0.0.0.0/$DISPLAY_HOST}"
printf '  orchestrator   http://%s\n' "${ORCH_BIND/0.0.0.0/$DISPLAY_HOST}"
printf '  previews       http://%s\n' "${PREVIEWS_BIND/0.0.0.0/$DISPLAY_HOST}"
printf '  frontend       http://%s:%s\n' "$DISPLAY_HOST" "$VITE_PORT"
printf '  logs           %s\n' "$LOG_DIR"

if [ "$OPEN" = "1" ]; then
  xdg-open "http://$WAIT_HOST:$VITE_PORT" 2>/dev/null || true
fi

wait "$VITE_PID"