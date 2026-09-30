import { useState, type FormEvent } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { Button, Form, Input } from '@canonical/react-components';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { navigateTo } from '../../lib/navigation';
import { getProviders, login } from '../../lib/api/auth';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys, routes } from '../../lib/routes';
import { S } from '../../strings/catalogue';
import { AppMark, AuthLayout, AuthSeparator } from './AuthLayout';
import { ProviderButtons } from './ProviderButtons';

interface LocationState {
  from?: string;
}

export function LoginPage() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const location = useLocation();
  const queryClient = useQueryClient();

  const from = (location.state as LocationState | null)?.from ?? routes.projects.list();
  const providers = useQuery({
    queryKey: queryKeys.authProviders(),
    queryFn: getProviders,
    retry: false,
    staleTime: 5 * 60_000,
  });

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    try {
      await login(email, password);
      await queryClient.invalidateQueries({ queryKey: queryKeys.session() });
      navigateTo(from);
    } catch (failure) {
      setError(getErrorMessage(failure));
    }
  };

  const registration = providers.data?.registration;

  return (
    <AuthLayout
      title={S.auth.signInTitle}
      footer={
        <>
          <Link className="p-link--quiet" to={routes.auth.forgot()}>
            {S.auth.forgotPassword}
          </Link>
          {registration === 'open' ? (
            <Link className="p-link--quiet" to={routes.auth.register()}>
              {S.auth.createAccount}
            </Link>
          ) : null}
        </>
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
          id="login-email"
          type="email"
          label={S.auth.email}
          autoComplete="username"
          value={email}
          onChange={(event) => setEmail(event.target.value)}
        />
        <Input
          id="login-password"
          type="password"
          label={S.auth.password}
          autoComplete="current-password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
        />
        <Button type="submit" appearance="positive" className="app-auth__submit">
          {S.auth.signIn}
        </Button>
      </Form>
      {(providers.data?.oidc ?? []).length > 0 ? (
        <>
          <AuthSeparator label={S.auth.or} />
          <ProviderButtons redirectTo={from} />
        </>
      ) : null}
    </AuthLayout>
  );
}
