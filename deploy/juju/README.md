# Menzi HA machine-charm deployment

This directory contains a Juju HA bundle and local charm sources for Menzi services.

## Local charms

Charm source roots:

- `deploy/juju/charms/core-api`
- `deploy/juju/charms/workspace`
- `deploy/juju/charms/orchestrator`
- `deploy/juju/charms/previews`
- `deploy/juju/charms/session-proxy`
- `deploy/juju/charms/llm-gateway`
- `deploy/juju/charms/git-integration`
- `deploy/juju/charms/notifications`
- `deploy/juju/charms/design-service`
- `deploy/juju/charms/workflow`
- `deploy/juju/charms/supervisor`
- `deploy/juju/charms/frontend`

Built artifacts:

- `deploy/juju/charms/*/*_amd64.charm`

## Build charms

Install charmcraft:

```bash
sudo snap install charmcraft --classic
```

Build all local charms:

```bash
for d in deploy/juju/charms/*; do
  (cd "$d" && sudo /snap/charmcraft/current/bin/charmcraft pack --destructive-mode)
done
```

## Bundle

`bundle-ha.yaml` references the local built charm artifacts directly via `./charms/<service>/<service>_amd64.charm`.
