# Local Dev Runbook

## Quick start

Run Postgres, NATS and the full dev stack (control plane on `:8080`,
orchestrator on `:8081`, previews on `:8095`, frontend on `:5173`):

```sh
infra/local/dev.sh
```

`dev.sh` is idempotent: it provisions Postgres and NATS via
`infra/local/setup.sh`, installs frontend dependencies on first run, builds the
backend binaries, starts the services and prints their URLs and log locations
under `target/dev/`. Ctrl+C stops everything. Add `--open` to open the
frontend in a browser. Override ports with `MENZI_API_BIND`,
`MENZI_ORCHESTRATOR_BIND`, `MENZI_PREVIEWS_BIND` or `MENZI_VITE_PORT`, and the
API the vite dev server proxies to with `MENZI_API_URL`.

## Provision

```sh
infra/local/setup.sh
```

The script is idempotent. It:

- installs `postgresql`, `postgresql-client` and `nats-server` via apt if missing
- starts the default Postgres cluster and creates the `menzi` role
  (password `menzi`, `CREATEDB`) plus the `menzi` and `menzi_test` databases
- enables JetStream in `/etc/nats-server.conf` with a persistent store at
  `/var/lib/nats-server/jetstream` and starts the `nats-server` service

Resulting endpoints:

| Service  | URL                                      |
| -------- | ---------------------------------------- |
| Postgres | `postgres://menzi:menzi@127.0.0.1:5432` |
| NATS     | `nats://127.0.0.1:4222`                  |

## Run all tests including integration

```sh
infra/local/test.sh
```

`test.sh` exports `MENZI_TEST_DATABASE_URL` and `MENZI_TEST_NATS_URL`, then runs
`cargo test --workspace`.

Without those variables, the integration tests in
`crates/db/tests/postgres.rs` and `crates/events/tests/nats.rs` skip
(CI-safe), so `cargo test --workspace` always passes on machines without the
services.

## Deployment references

- `infra/terraform` — LXD deployment via Terraform (one instance per service,
  N preview replicas). Requires Terraform >= 1.6 and LXD with a reachable
  remote.
- `infra/juju` — Juju machine-model bundle deploying PostgreSQL and NATS from
  Charmhub plus one unit per Menzi service.
- Previews run as a standalone service (`menzi-previews`, default
  `0.0.0.0:8095`). The control plane proxies `/api/v1/previews*` and
  `/api/v1/projects/{project_id}/previews` to it via `MENZI_PREVIEWS_URL`.

## What the integration tests cover

- `crates/db`: connecting to a fresh database applies all `crates/db/migrations`
  (`_sqlx_migrations`), creates the full expected schema, and `health_check` succeeds.
- `crates/events`: `NatsEventBus` round-trips typed `Event<T>` payloads through a
  JetStream stream with a durable pull consumer, preserves ordering, and
  `ensure_stream` is idempotent.