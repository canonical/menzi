import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import { Spinner } from '@canonical/react-components';
import { useQueryClient } from '@tanstack/react-query';
import { queryKeys, routes } from '../../lib/routes';
import { navigateTo } from '../../lib/navigation';
import { S } from '../../strings/catalogue';

export function AuthCallbackPage() {
  const location = useLocation();
  const queryClient = useQueryClient();

  useEffect(() => {
    const params = new URLSearchParams(location.search);
    const failed = params.get('error');
    const target = params.get('target');

    if (failed) {
      navigateTo(routes.auth.login());
      return;
    }

    queryClient.invalidateQueries({ queryKey: queryKeys.session() });
    navigateTo(target ?? routes.projects.list());
  }, [location.search, queryClient]);

  return (
    <div className="app-full-page-loading">
      <Spinner text={S.auth.completingSignIn} />
    </div>
  );
}
