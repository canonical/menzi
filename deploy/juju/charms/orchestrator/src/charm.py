from ops import ActiveStatus, BlockedStatus, CharmBase, main


class MenziServiceCharm(CharmBase):
    def __init__(self, *args):
        super().__init__(*args)
        self.framework.observe(self.on.install, self._reconcile)
        self.framework.observe(self.on.start, self._reconcile)
        self.framework.observe(self.on.config_changed, self._reconcile)
        self.framework.observe(self.on.update_status, self._reconcile)
        self.framework.observe(self.on.leader_elected, self._reconcile)
        for endpoint in self.meta.requires.keys():
            relation_events = getattr(self.on, endpoint)
            self.framework.observe(relation_events.relation_joined, self._reconcile)
            self.framework.observe(relation_events.relation_changed, self._reconcile)
            self.framework.observe(relation_events.relation_broken, self._reconcile)

    def _reconcile(self, _):
        waiting = []
        for endpoint in self.meta.requires.keys():
            if not self.model.relations.get(endpoint):
                waiting.append(endpoint)
        if waiting:
            waiting.sort()
            self.unit.status = BlockedStatus('waiting for relations: ' + ','.join(waiting))
            return
        self.unit.status = ActiveStatus('ready')


if __name__ == '__main__':
    main(MenziServiceCharm)
