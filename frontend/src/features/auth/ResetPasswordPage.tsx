import { useState, type FormEvent } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { Button, Form, Input } from '@canonical/react-components';
import { routes } from '../../lib/routes';
import { S } from '../../strings/catalogue';
import { AppMark, AuthLayout } from './AuthLayout';

export function ResetPasswordPage() {
  const [search] = useSearchParams();
  const token = search.get('token') ?? '';
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');

  if (!token) {
    return (
      <AuthLayout title={S.auth.resetTitle}>
        <AppMark />
        <p data-testid="reset-no-token">{S.auth.resetMissingToken}</p>
        <Link className="p-link--quiet" to={routes.auth.forgot()}>
          {S.auth.forgotPassword}
        </Link>
      </AuthLayout>
    );
  }

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
  };

  return (
    <AuthLayout
      title={S.auth.resetTitle}
      footer={
        <Link className="p-link--quiet" to={routes.auth.login()}>
          {S.auth.backToSignIn}
        </Link>
      }
    >
      <AppMark />
      <Form onSubmit={handleSubmit}>
        <Input
          id="reset-password"
          type="password"
          label={S.auth.newPassword}
          autoComplete="new-password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
        <Input
          id="reset-confirm"
          type="password"
          label={S.auth.confirmPassword}
          autoComplete="new-password"
          value={confirm}
          onChange={(event) => setConfirm(event.target.value)}
        />
        <Button type="submit" appearance="positive" className="app-auth__submit">
          {S.auth.saveNewPassword}
        </Button>
      </Form>
    </AuthLayout>
  );
}
