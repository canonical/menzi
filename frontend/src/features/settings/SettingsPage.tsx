import { ChangeEvent, FormEvent, useState } from 'react';
import {
  Button,
  Form,
  Icon,
  Input,
  Spinner,
  Switch,
  Tabs,
  useNotify,
} from '@canonical/react-components';
import { S } from '../../strings/catalogue';

const SETTINGS_TABS = [
  { id: 'profile', label: S.settings.tabs.profile },
  { id: 'models', label: S.settings.tabs.modelAccounts },
  { id: 'notifications', label: S.settings.tabs.notifications },
];

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function SettingsPage() {
  const [activeTab, setActiveTab] = useState(SETTINGS_TABS[0].id);
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [nameError, setNameError] = useState<string | null>(null);
  const [emailError, setEmailError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [webPush, setWebPush] = useState(true);
  const [emailEnabled, setEmailEnabled] = useState(false);
  const [chat, setChat] = useState(false);
  const notify = useNotify();

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const nextNameError = name.trim() ? null : S.settings.profile.errors.nameRequired;
    const nextEmailError = EMAIL_PATTERN.test(email)
      ? null
      : S.settings.profile.errors.emailInvalid;
    setNameError(nextNameError);
    setEmailError(nextEmailError);
    if (nextNameError || nextEmailError) return;
    setSaving(true);
    window.setTimeout(() => {
      setSaving(false);
      notify.success(S.settings.profile.saved);
    }, 600);
  };

  return (
    <div>
      <h2 className="p-heading--3">{S.settings.title}</h2>
      <Tabs
        links={SETTINGS_TABS.map((tab) => ({
          label: tab.label,
          onClick: () => setActiveTab(tab.id),
          active: activeTab === tab.id,
        }))}
      />
      <div className="p-card">
        <div className="p-card__content">
          {activeTab === 'profile' && (
            <Form onSubmit={handleSubmit}>
              <Input
                id="settings-name"
                label={S.settings.profile.nameLabel}
                type="text"
                required
                value={name}
                error={nameError}
                onChange={(event) => setName(event.target.value)}
              />
              <Input
                id="settings-email"
                label={S.settings.profile.emailLabel}
                type="email"
                required
                value={email}
                error={emailError}
                onChange={(event) => setEmail(event.target.value)}
              />
              <Button disabled={saving} appearance="positive" type="submit">
                {saving ? <Spinner text={S.dataState.loading} /> : S.settings.profile.save}
              </Button>
            </Form>
          )}
          {activeTab === 'models' && (
            <div>
              <p>{S.settings.modelAccounts.intro}</p>
              <Button appearance="positive">
                <Icon name="plus" />
                {S.settings.modelAccounts.addAccount}
              </Button>
            </div>
          )}
          {activeTab === 'notifications' && (
            <div>
              <p>{S.settings.notifications.intro}</p>
              <Switch
                label={S.settings.notifications.webPush}
                checked={webPush}
                onChange={(event: ChangeEvent<HTMLInputElement>) =>
                  setWebPush(event.target.checked)
                }
              />
              <Switch
                label={S.settings.notifications.email}
                checked={emailEnabled}
                onChange={(event: ChangeEvent<HTMLInputElement>) =>
                  setEmailEnabled(event.target.checked)
                }
              />
              <Switch
                label={S.settings.notifications.chat}
                checked={chat}
                onChange={(event: ChangeEvent<HTMLInputElement>) => setChat(event.target.checked)}
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
