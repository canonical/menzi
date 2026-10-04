#!/usr/bin/env bash
# Build the LXD container a workspace is cloned from.
#
# One template for every project, project checked out at provision time. The
# template carries the toolchain, opencode, and an empty /workspace owned by the
# agent user, so a cloned workspace is immediately writable by the agent. It is
# deliberately *not* a host bind mount: a host directory appears inside the
# container owned by nobody, and the agent cannot write to it.
#
# The result is a *container*, not a published image, because both the workspace
# and the preview driver copy it with LXD's source type "copy".
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

NAME="${MENZI_WORKSPACE_TEMPLATE:-mz-workspace}"
BUILD="${MENZI_WORKSPACE_BUILD:-mz-workspace-build}"
IMAGE="${MENZI_WORKSPACE_IMAGE:-ee3016f85cc4}"
PROFILE="${MENZI_WORKSPACE_PROFILE:-menzi}"
AGENT_USER="${MENZI_WORKSPACE_USER:-menzi}"
AGENT_HOME="/home/$AGENT_USER"
PORT="${MENZI_WORKSPACE_PORT:-17999}"
PLATFORM_CONTAINER="${MENZI_PLATFORM_CONTAINER:-menzi-supervisor}"
PLATFORM_OPENCODE="${MENZI_PLATFORM_OPENCODE:-/root/.opencode/bin/opencode}"
STAGED_BINARY=/tmp/mz-workspace-opencode

if [ "$(id -u)" -ne 0 ]; then
  echo "error: run as root, lxc needs it" >&2
  exit 1
fi

if lxc info "$NAME" >/dev/null 2>&1; then
  echo "$NAME already exists; delete it first (lxc delete $NAME) to rebuild" >&2
  exit 1
fi

cleanup() {
  rm -f "$STAGED_BINARY"
  lxc delete --force "$BUILD" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# The opencode in a workspace has to be the one the rest of the platform speaks
# to, or the proxy and the frontend hit a different API than the workspace does.
# opencode 2 also refuses unauthenticated requests on a non-loopback bind, which
# nothing in menzi is wired for, so version drift here breaks the product rather
# than just the template.
resolve_opencode() {
  if [ -n "${MENZI_WORKSPACE_OPENCODE_BIN:-}" ] && [ -f "$MENZI_WORKSPACE_OPENCODE_BIN" ]; then
    cp "$MENZI_WORKSPACE_OPENCODE_BIN" "$STAGED_BINARY"
    echo "opencode source: $MENZI_WORKSPACE_OPENCODE_BIN"
    return
  fi
  if lxc info "$PLATFORM_CONTAINER" >/dev/null 2>&1; then
    if lxc file pull "$PLATFORM_CONTAINER$PLATFORM_OPENCODE" "$STAGED_BINARY" 2>/dev/null; then
      echo "opencode source: $PLATFORM_CONTAINER$PLATFORM_OPENCODE"
      return
    fi
  fi
  if [ -f /usr/local/bin/opencode ]; then
    cp /usr/local/bin/opencode "$STAGED_BINARY"
    echo "opencode source: /usr/local/bin/opencode"
    return
  fi
  echo "error: no opencode binary found; set MENZI_WORKSPACE_OPENCODE_BIN" >&2
  exit 1
}

echo "launching $BUILD from $IMAGE"
lxc launch "$IMAGE" "$BUILD" --profile "$PROFILE"

lxc exec "$BUILD" -- cloud-init status --wait >/dev/null 2>&1 || true

echo "installing the toolchain"
lxc exec "$BUILD" -- bash -c 'export DEBIAN_FRONTEND=noninteractive; apt-get update -qq && apt-get install -y -qq --no-install-recommends ca-certificates curl git ripgrep jq build-essential unzip less procps htop iproute2 >/dev/null'

echo "creating the agent user"
lxc exec "$BUILD" -- bash -c "useradd -m -s /bin/bash $AGENT_USER"

echo "installing opencode"
resolve_opencode
lxc file push "$STAGED_BINARY" "$BUILD/usr/local/bin/opencode" --mode=0755 >/dev/null
WORKSPACE_VERSION="$(lxc exec "$BUILD" -- opencode --version)"
echo "opencode $WORKSPACE_VERSION"
if lxc info "$PLATFORM_CONTAINER" >/dev/null 2>&1; then
  PLATFORM_VERSION="$(lxc exec "$PLATFORM_CONTAINER" -- "$PLATFORM_OPENCODE" --version 2>/dev/null || true)"
  if [ -n "$PLATFORM_VERSION" ] && [ "$PLATFORM_VERSION" != "$WORKSPACE_VERSION" ]; then
    echo "warning: workspace opencode $WORKSPACE_VERSION differs from $PLATFORM_CONTAINER's $PLATFORM_VERSION" >&2
  fi
fi

echo "laying out the workspace directory"
lxc exec "$BUILD" -- bash -c "mkdir -p $AGENT_HOME/.config/opencode /workspace && chown -R $AGENT_USER:$AGENT_USER $AGENT_HOME /workspace && chmod 0775 /workspace"
lxc file push "$ROOT/infra/local/opencode.json" "$BUILD/tmp/opencode.json" --mode=0644
lxc exec "$BUILD" -- bash -c "install -o $AGENT_USER -g $AGENT_USER -m 0644 /tmp/opencode.json $AGENT_HOME/.config/opencode/opencode.json && rm /tmp/opencode.json"

echo "installing the opencode service"
lxc exec "$BUILD" -- bash -c "cat > /etc/systemd/system/opencode.service <<'UNIT'
[Unit]
Description=opencode server for a menzi workspace
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$AGENT_USER
WorkingDirectory=/workspace
Environment=HOME=$AGENT_HOME
Environment=OPENCODE_CONFIG_DIR=$AGENT_HOME/.config/opencode
Environment=OPENCODE_SERVER_USERNAME=opencode
ExecStart=/usr/local/bin/opencode serve --hostname 0.0.0.0 --port $PORT
Restart=always
RestartSec=3
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
UNIT"

echo "verifying opencode starts and answers"
lxc exec "$BUILD" -- systemctl daemon-reload
lxc exec "$BUILD" -- systemctl enable --now opencode.service
if ! lxc exec "$BUILD" -- bash -c "for i in \$(seq 1 30); do code=\$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:$PORT/api/model || true); [ \"\$code\" = \"401\" ] && exit 0; sleep 1; done; exit 1"; then
  lxc exec "$BUILD" -- journalctl -u opencode --no-pager | tail -30
  echo "error: opencode did not answer with auth required on $PORT" >&2
  exit 1
fi

echo "stopping $BUILD"
lxc stop "$BUILD"

echo "renaming $BUILD to $NAME"
lxc move "$BUILD" "$NAME"
trap - EXIT
rm -f "$STAGED_BINARY"

echo "built $NAME"
lxc list "$NAME"
