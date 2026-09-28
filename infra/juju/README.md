# Menzi on Juju

Deploys the Menzi platform onto an existing Juju controller using a machine
model. The bundle in `bundle.yaml` provisions the shared data stores from
Charmhub and one (or more) unit per Menzi service.

## Layout

| Application      | Units | Purpose                              |
| ---------------- | ----- | ------------------------------------ |
| `postgresql`     | 1     | Control-plane and audit state store  |
| `nats`           | 1     | Event bus and HA lease coordination  |
| `core-api`       | 1     | Control plane (orgs, previews proxy) |
| `llm-gateway`    | 1     | Provider gateway with tenant budgets |
| `session-proxy`  | 1     | Reverse proxy for supervisor sessions|
| `orchestrator`   | 1+    | Environment lifecycle over LXD      |
| `supervisor`     | 1+    | opencode agent runtime per session   |
| `previews`       | 2     | Preview manager replicas (HA)        |

## Deploy

```sh
juju add-model menzi
juju deploy ./bundle.yaml
```

## Replacing placeholders

The Menzi service units currently use the `ubuntu` base charm so the bundle
deploys out of the box. For production, replace each service unit with the
packaged Menzi charm:

```sh
juju deploy ./charms/core-api
juju deploy ./charms/orchestrator
# repeat for llm-gateway, session-proxy, supervisor, previews
```

Each packaged charm is expected to expose configuration keys for
`database-url`, `nats-url`, `lxd-url`, `lxd-cert-path`, and `lxd-key-path`,
mirroring the `menzi-common` `Config` environment variables.