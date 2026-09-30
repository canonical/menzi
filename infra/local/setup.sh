#!/usr/bin/env bash
set -euo pipefail

if ! command -v psql >/dev/null 2>&1; then
  sudo apt-get update -qq
  sudo apt-get install -y -qq postgresql postgresql-client nats-server
fi

if ! pg_isready -h 127.0.0.1 -p 5432 >/dev/null 2>&1; then
  sudo systemctl start postgresql
fi

sudo -u postgres psql -v ON_ERROR_STOP=1 <<'SQL'
DO $$
BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'menzi') THEN
    CREATE ROLE menzi WITH LOGIN PASSWORD 'menzi' CREATEDB;
  END IF;
END
$$;
SQL

if ! sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname = 'menzi'" | grep -q 1; then
  sudo -u postgres createdb -O menzi menzi
fi

if ! sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname = 'menzi_test'" | grep -q 1; then
  sudo -u postgres createdb -O menzi menzi_test
fi

sudo mkdir -p /var/lib/nats-server/jetstream
sudo tee /etc/nats-server.conf >/dev/null <<'EOF'
host: 127.0.0.1
port: 4222

jetstream {
  store_dir: "/var/lib/nats-server/jetstream"
  max_mem_store: 1G
  max_file_store: 10G
}
EOF
sudo chown -R nats:nats /var/lib/nats-server
sudo systemctl enable --now nats-server
sudo systemctl restart nats-server

# The services talk to LXD over TLS with a client certificate. A snap LXD trusts
# nothing by default and exposes no certificate in ~/.config/lxc, so mint one and
# trust it. Harmless if it already exists or is already trusted.
if command -v lxc >/dev/null 2>&1; then
  mkdir -p "$HOME/.config/lxc"
  if [ ! -f "$HOME/.config/lxc/client.crt" ] || [ ! -f "$HOME/.config/lxc/client.key" ]; then
    openssl req -x509 -newkey rsa:2048 -nodes \
      -keyout "$HOME/.config/lxc/client.key" \
      -out "$HOME/.config/lxc/client.crt" \
      -days 3650 -subj "/CN=menzi" -addext "extendedKeyUsage=clientAuth" 2>/dev/null
    echo "generated an lxd client certificate in $HOME/.config/lxc"
  fi
  fp="$(openssl x509 -in "$HOME/.config/lxc/client.crt" -noout -fingerprint -sha256 | cut -d= -f2)"
  if ! sudo lxc config trust list --format csv 2>/dev/null | cut -d, -f5 | tr -d ' ' | grep -qF "$fp"; then
    sudo lxc config trust add "$HOME/.config/lxc/client.crt" && echo "trusted the lxd client certificate"
  fi
  export MENZI_LXD_CERT_PATH="${MENZI_LXD_CERT_PATH:-$HOME/.config/lxc/client.crt}"
  export MENZI_LXD_KEY_PATH="${MENZI_LXD_KEY_PATH:-$HOME/.config/lxc/client.key}"
  export MENZI_LXD_URL="${MENZI_LXD_URL:-https://127.0.0.1:8443}"
fi