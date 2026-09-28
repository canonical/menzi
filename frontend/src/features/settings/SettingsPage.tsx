import { useState } from 'react';

export function SettingsPage() {
  const [activeTab, setActiveTab] = useState('profile');

  return (
    <div>
      <h2 className="p-heading--3">Settings</h2>
      <div className="p-tabs">
        <ul className="p-tabs__list">
          <li className="p-tabs__item">
            <button
              className={`p-tabs__link ${activeTab === 'profile' ? 'is-active' : ''}`}
              onClick={() => setActiveTab('profile')}
            >
              Profile
            </button>
          </li>
          <li className="p-tabs__item">
            <button
              className={`p-tabs__link ${activeTab === 'models' ? 'is-active' : ''}`}
              onClick={() => setActiveTab('models')}
            >
              Model accounts
            </button>
          </li>
          <li className="p-tabs__item">
            <button
              className={`p-tabs__link ${activeTab === 'notifications' ? 'is-active' : ''}`}
              onClick={() => setActiveTab('notifications')}
            >
              Notifications
            </button>
          </li>
        </ul>
      </div>
      <div className="p-card">
        <div className="p-card__content">
          {activeTab === 'profile' && (
            <form className="p-form">
              <div className="p-form__group">
                <label className="p-form__label">Name</label>
                <input type="text" className="p-form__control" />
              </div>
              <div className="p-form__group">
                <label className="p-form__label">Email</label>
                <input type="email" className="p-form__control" />
              </div>
              <button type="submit" className="p-button--positive">Save</button>
            </form>
          )}
          {activeTab === 'models' && (
            <div>
              <h4>Model accounts</h4>
              <p>Add or manage your model provider accounts.</p>
              <button className="p-button--positive">Add account</button>
            </div>
          )}
          {activeTab === 'notifications' && (
            <div>
              <h4>Notification preferences</h4>
              <form className="p-form">
                <div className="p-form__group">
                  <label className="p-form__label">
                    <input type="checkbox" /> Web push
                  </label>
                </div>
                <div className="p-form__group">
                  <label className="p-form__label">
                    <input type="checkbox" /> Email
                  </label>
                </div>
                <div className="p-form__group">
                  <label className="p-form__label">
                    <input type="checkbox" /> Chat
                  </label>
                </div>
                <button type="submit" className="p-button--positive">Save</button>
              </form>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
