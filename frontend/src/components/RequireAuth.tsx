import { Navigate, Outlet, useLocation } from 'react-router-dom';
import { Spinner } from '@canonical/react-components';
import { useSession } from '../stores/useSession';
import { routes } from '../lib/routes';
import { S } from '../strings/catalogue';

function FullPageSpinner() {
  return (
    <div className="app-full-page-loading">
      <Spinner text={S.dataState.loading} />
    </div>
  );
}

export function RequireAuth() {
  const { status } = useSession();
  const location = useLocation();

  if (status === 'loading') return <FullPageSpinner />;

  if (status === 'anonymous') {
    return <Navigate to={routes.auth.login()} replace state={{ from: location.pathname + location.search }} />;
  }

  return <Outlet />;
}

export function RequireAnonymous() {
  const { status } = useSession();

  if (status === 'loading') return <FullPageSpinner />;

  if (status === 'authenticated') {
    return <Navigate to={routes.projects.list()} replace />;
  }

  return <Outlet />;
}
