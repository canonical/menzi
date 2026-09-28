import { useState } from 'react';

export function AdminPage() {
  const [activeTab, setActiveTab] = useState('users');

  return (
    <div>
      <h2 className="p-heading--3">Admin</h2>
      <div className="p-tabs">
        <ul className="p-tabs__list">
          <li className="p-tabs__item">
            <button
              className={`p-tabs__link ${activeTab === 'users' ? 'is-active' : ''}`}
              onClick={() => setActiveTab('users')}
            >
              Users
            </button>
          </li>
          <li className="p-tabs__item">
            <button
              className={`p-tabs__link ${activeTab === 'quotas' ? 'is-active' : ''}`}
              onClick={() => setActiveTab('quotas')}
            >
              Quotas
            </button>
          </li>
          <li className="p-tabs__item">
            <button
              className={`p-tabs__link ${activeTab === 'billing' ? 'is-active' : ''}`}
              onClick={() => setActiveTab('billing')}
            >
              Billing
            </button>
          </li>
        </ul>
      </div>
      <div className="p-card">
        <div className="p-card__content">
          {activeTab === 'users' && <p>User management</p>}
          {activeTab === 'quotas' && <p>Quota management</p>}
          {activeTab === 'billing' && <p>Billing management</p>}
        </div>
      </div>
    </div>
  );
}
