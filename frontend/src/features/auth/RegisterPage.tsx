import { useState, type FormEvent } from 'react';
import { Link } from 'react-router-dom';
import { Button, Form, Input } from '@canonical/react-components';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { navigateTo } from '../../lib/navigation';
import { getProviders, register } from '../../lib/api/auth';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys, routes } from '../../lib/routes';
import { S } from '../../strings/catalogue';
import { AppMark, AuthLayout } from './AuthLayout';

const MINIMUM_LENGTH = 12;

export function RegisterPage() {
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const providers = useQuery({
    queryKey: queryKeys.authProviders(),
    queryFn: getProviders,
    retry: false,
    staleTime: 5 * 60_000,
  });

  if (providers.data && providers.data.registration !== 'open') {
    return (
      <AuthLayout title={S.auth.registerTitle}>
        <AppMark />
        <p data-testid="registration-closed">{S.auth.registrationClosed}</p>
        <Link className="p-link--quiet" to={routes.auth.login()}>
          {S.auth.backToSignIn}
        </Link>
      </AuthLayout>
    );
  }

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    if (password.length < MINIMUM_LENGTH) {
      setError(S.auth.passwordTooShort.replace('{minimum}', String(MINIMUM_LENGTH)));
      return;
    }
    if (password !== confirm) {
      setError(S.auth.passwordsDoNotMatch);
      return;
    }
    try {
      await register(email, name, password);
      await queryClient.invalidateQueries({ queryKey: queryKeys.session() });
      navigateTo(routes.projects.list());
    } catch (failure) {
      setError(getErrorMessage(failure));
    }
  };

  return (
    <AuthLayout
      title={S.auth.registerTitle}
      footer={
        <Link className="p-link--quiet" to={routes.auth.login()}>
          {S.auth.backToSignIn}
        </Link>
      }
    >
      <AppMark />
      <Form onSubmit={handleSubmit}>
        {error ? (
          <p className="p-form-validation__message" data-testid="auth-error" role="alert">
            {error}
          </p>
        ) : null}
        <Input
          id="register-name"
          type="text"
          label={S.auth.name}
          autoComplete="name"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        <Input
          id="register-email"
          type="email"
          label={S.auth.email}
          autoComplete="username"
          value={email}
          onChange={(event) => setEmail(event.target.value)}
        />
        <Input
          id="register-password"
          type="password"
          label={S.auth.password}
          autoComplete="new-password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
        <Input
          id="register-confirm"
          type="password"
          label={S.auth.confirmPassword}
          autoComplete="new-password"
          value={confirm}
          onChange={(event) => setConfirm(event.target.value)}
        />
        <p className="p-form-help-text" data-testid="password-length">
          {S.auth.passwordLength.replace(
            '{minimum}',
            String(MINIMUM_LENGTH),
          ).replace('{current}', String(password.length))}
        </p>
        <Button
          type="submit"
          appearance="positive"
          className="app-auth__submit"
        >
          {S.auth.createAccount}
        </Button>
      </Form>
    </AuthLayout>
  );
}
