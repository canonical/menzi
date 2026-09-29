import { useState } from 'react';
import { Tabs } from '@canonical/react-components';
import { S } from '../../strings/catalogue';

const ADMIN_TABS = [
  { id: 'users', label: S.admin.tabs.users },
  { id: 'quotas', label: S.admin.tabs.quotas },
  { id: 'billing', label: S.admin.tabs.billing },
];

export function AdminPage() {
  const [activeTab, setActiveTab] = useState(ADMIN_TABS[0].id);

  return (
    <div>
      <h2 className="p-heading--3">{S.admin.title}</h2>
      <Tabs
        links={ADMIN_TABS.map((tab) => ({
          label: tab.label,
          onClick: () => setActiveTab(tab.id),
          active: activeTab === tab.id,
        }))}
      />
      <div className="p-card">
        <div className="p-card__content">
          {activeTab === 'users' && <p>{S.admin.bodies.users}</p>}
          {activeTab === 'quotas' && <p>{S.admin.bodies.quotas}</p>}
          {activeTab === 'billing' && <p>{S.admin.bodies.billing}</p>}
        </div>
      </div>
    </div>
  );
}