import { useState, type FormEvent } from 'react';
import { Button, Form, Input, Spinner, useNotify } from '@canonical/react-components';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  changePassword,
  listDevices,
  revokeDevice,
  type Device,
} from '../../lib/api/auth';
import { getErrorMessage } from '../../lib/api/errors';
import { formatDateTime } from '../../lib/format/time';
import { queryKeys } from '../../lib/routes';
import { S } from '../../strings/catalogue';
import { useAuthStore } from '../../stores/auth';

const MINIMUM_LENGTH = 12;

export function AccountSection() {
  const user = useAuthStore((state) => state.user);
  const notify = useNotify();
  const queryClient = useQueryClient();

  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);

  const devices = useQuery({
    queryKey: queryKeys.devices(),
    queryFn: listDevices,
    retry: false,
  });

  const change = useMutation({
    mutationFn: () => changePassword(current, next),
    onSuccess: () => {
      setCurrent('');
      setNext('');
      setConfirm('');
      setError(null);
      queryClient.invalidateQueries({ queryKey: queryKeys.devices() });
      notify.success(S.auth.passwordChanged);
    },
    onError: (failure) => setError(getErrorMessage(failure)),
  });

  const revoke = useMutation({
    mutationFn: (deviceId: string) => revokeDevice(deviceId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.devices() });
      notify.success(S.auth.deviceRevoked);
    },
    onError: (failure) => notify.failure(S.auth.deviceRevokeFailed, failure, getErrorMessage(failure)),
  });

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    if (next.length < MINIMUM_LENGTH) {
      setError(S.auth.passwordTooShort.replace('{minimum}', String(MINIMUM_LENGTH)));
      return;
    }
    if (next !== confirm) {
      setError(S.auth.passwordsDoNotMatch);
      return;
    }
    change.mutate();
  };

  return (
    <div className="app-account" data-testid="account-section">
      <section>
        <h3 className="p-heading--4">{S.auth.accountHeading}</h3>
        <p data-testid="account-email">{user?.email ?? S.auth.noEmail}</p>
      </section>

      <section>
        <h3 className="p-heading--4">{S.auth.passwordHeading}</h3>
        {user?.has_password !== false ? (
          <Form onSubmit={handleSubmit}>
            {error ? (
              <p className="p-form-validation__message" role="alert" data-testid="account-error">
                {error}
              </p>
            ) : null}
            <Input
              id="account-current-password"
              type="password"
              label={S.auth.currentPassword}
              autoComplete="current-password"
              value={current}
              onChange={(event) => setCurrent(event.target.value)}
            />
            <Input
              id="account-new-password"
              type="password"
              label={S.auth.newPassword}
              autoComplete="new-password"
              value={next}
              onChange={(event) => setNext(event.target.value)}
            />
            <Input
              id="account-confirm-password"
              type="password"
              label={S.auth.confirmPassword}
              autoComplete="new-password"
              value={confirm}
              onChange={(event) => setConfirm(event.target.value)}
            />
            <Button type="submit" appearance="positive" disabled={change.isPending}>
              {change.isPending ? <Spinner text={S.dataState.loading} /> : S.auth.saveNewPassword}
            </Button>
          </Form>
        ) : (
          <p data-testid="no-password">{S.auth.noPassword}</p>
        )}
      </section>

      <section>
        <h3 className="p-heading--4">{S.auth.devicesHeading}</h3>
        <ul className="app-account__devices" data-testid="device-list">
          {(devices.data ?? []).map((device) => (
            <li key={device.id}>
              <DeviceRow device={device} onRevoke={() => revoke.mutate(device.id)} />
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

function DeviceRow({ device, onRevoke }: { device: Device; onRevoke: () => void }) {
  return (
    <div className="app-account__device">
      <div>
        <span className="app-account__device-provider">
          {device.current ? S.auth.thisDevice : device.user_agent ?? S.auth.unknownDevice}
        </span>
        <span className="u-text--muted u-no-margin--bottom">
          {S.auth.lastSeen.replace(
            '{when}',
            formatDateTime(new Date(device.last_seen_at).toISOString()),
          )}
        </span>
      </div>
      {device.current ? null : (
        <Button appearance="link" onClick={onRevoke}>
          {S.auth.revoke}
        </Button>
      )}
    </div>
  );
}
