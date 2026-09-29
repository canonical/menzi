# Local Dev Runbook

## Quick start

Run Postgres, NATS and the full dev stack (control plane on `:8080`,
orchestrator on `:8081`, previews on `:8095`, session proxy on `:8082`, llm
gateway on `:8083`, frontend on `:5173`):

```sh
infra/local/dev.sh
```

`dev.sh` is idempotent: it provisions Postgres and NATS via
`infra/local/setup.sh`, installs frontend dependencies on first run, builds the
backend binaries, starts the services and prints their URLs and log locations
under `target/dev/`. Ctrl+C stops everything. Add `--open` to open the
frontend in a browser. Bind all services to a specific host with
`--host 0.0.0.0` (or `MENZI_DEV_HOST`). Override ports with `MENZI_API_BIND`,
`MENZI_ORCHESTRATOR_BIND`, `MENZI_PREVIEWS_BIND`,
`MENZI_SESSION_PROXY_BIND`, `MENZI_LLM_GATEWAY_BIND` or `MENZI_VITE_PORT`, and
the backends the vite dev server proxies to with `MENZI_API_URL`,
`MENZI_ORCHESTRATOR_URL` and `MENZI_SESSION_PROXY_URL`.

Each service is probed for an expected status code rather than mere
reachability, so a route that answers `4xx` for a health check fails the probe
instead of being reported as healthy:

| Service | Probe | Expected |
| --- | --- | --- |
| control plane | `GET /health` | `200` |
| orchestrator | `GET /api/env/health` | `200` |
| previews | `GET /api/v1/previews` | `405` |
| session proxy | `GET /health` | `200` |
| llm gateway | `GET /v1/models` | `200` |
| frontend | `GET /` | `200` |

The previews probe expects `405` deliberately: that path only accepts `POST`,
and a `405` proves the route is registered and the service is answering.

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

## Reading a route's behaviour from a status code

Do not infer that an endpoint is missing from a non-2xx response. Read the
route list instead.

| Code | Meaning |
| --- | --- |
| `404` | No such route. The path is not registered. |
| `405` | The route exists; the method is wrong. `GET /api/v1/previews` answers `405` because only `POST` is registered on that path. |
| `415` | The route exists; the request media type is wrong. `GET /api/env/status` answers `415` to a bodyless probe because it takes a JSON body. |
| `000` | `curl` could not connect. Usually a wrong port or a service that is not listening, not an application-level answer. |

`dev.sh` prints whatever code it gets, so lines like
`orchestrator up (415) at …` mean the service is healthy and reachable. They are
not errors to chase.

`GET /api/env/status` takes a JSON body, which is legal but defeats a generic
health probe. `GET /api/env/health` is the bodyless route to use for liveness.

The authoritative list of routes is `docs/backend-inventory.md`, generated from
the Rust sources. Regenerate it with `npm run inventory` from `frontend/`. It is
checked in CI: adding a route without regenerating fails the frontend test suite,
as does adding a frontend call to a route the backend does not register.