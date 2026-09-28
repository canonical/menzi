# Menzi on LXD via Terraform

Provisions a single-node LXD deployment of the Menzi platform: a bridge
network, a storage pool, a shared profile, and one instance per service,
plus N horizontal replicas of the preview service.

## Requirements

- LXD 5.x or newer, with the LXD remote reachable from where `terraform` runs
- Terraform >= 1.6

## Usage

```sh
terraform init
terraform plan
terraform apply
```

The `lxd` provider generates a client certificate on first run and can trust
the remote certificate automatically.

## Layout

| Service                 | Instance                  |
| ----------------------- | ------------------------- |
| PostgreSQL              | `menzi-postgres`          |
| NATS + JetStream        | `menzi-nats`              |
| Control plane (core-api) | `menzi-core-api`         |
| LLM gateway             | `menzi-llm-gateway`       |
| Session proxy           | `menzi-session-proxy`     |
| Orchestrator            | `menzi-orchestrator`      |
| Supervisor              | `menzi-supervisor`        |
| Previews (N replicas)   | `menzi-previews-{1..N}`   |

After the instances exist, provision the service binaries inside each
instance (e.g. via a config management layer) and point them at each other
using the `menzinet` bridge network (`10.120.0.0/24`).