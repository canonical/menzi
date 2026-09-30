import { useState, type FormEvent } from 'react';
import { Link } from 'react-router-dom';
import { Button, Form, Input } from '@canonical/react-components';
import { requestPasswordReset } from '../../lib/api/auth';
import { getErrorMessage } from '../../lib/api/errors';
import { routes } from '../../lib/routes';
import { S } from '../../strings/catalogue';
import { AppMark, AuthLayout } from './AuthLayout';

export function ForgotPasswordPage() {
  const [email, setEmail] = useState('');
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setError(null);
    try {
      await requestPasswordReset(email);
      setSent(true);
    } catch (failure) {
      setError(getErrorMessage(failure));
    }
  };

  return (
    <AuthLayout
      title={S.auth.forgotTitle}
      footer={
        <Link className="p-link--quiet" to={routes.auth.login()}>
          {S.auth.backToSignIn}
        </Link>
      }
    >
      <AppMark />
      {sent ? (
        <p data-testid="reset-sent">{S.auth.resetSent}</p>
      ) : (
        <Form onSubmit={handleSubmit}>
          {error ? (
            <p className="p-form-validation__message" role="alert">
              {error}
            </p>
          ) : null}
          <Input
            id="forgot-email"
            type="email"
            label={S.auth.email}
            autoComplete="username"
            value={email}
            onChange={(event) => setEmail(event.target.value)}
          />
          <Button type="submit" appearance="positive" className="app-auth__submit">
            {S.auth.sendResetLink}
          </Button>
        </Form>
      )}
    </AuthLayout>
  );
}
