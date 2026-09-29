import { ReactNode } from 'react';
import { Button, EmptyState, Icon, Spinner } from '@canonical/react-components';
import { S } from '../strings/catalogue';

interface DataStateProps {
  loading: boolean;
  error: string | null;
  empty: boolean;
  emptyTitle?: string;
  emptyBody?: string;
  emptyIcon?: string;
  onRetry?: () => void;
  children: ReactNode;
}

export function DataState({
  loading,
  error,
  empty,
  emptyTitle,
  emptyBody,
  emptyIcon = 'help',
  onRetry,
  children,
}: DataStateProps) {
  if (loading) {
    return (
      <div className="u-align--center u-vertically-center">
        <Spinner text={S.dataState.loading} />
      </div>
    );
  }

  if (error) {
    return (
      <EmptyState
        title={S.dataState.errorTitle}
        image={<Icon name="error" />}
      >
        <p>{error}</p>
        {onRetry && (
          <Button appearance="positive" onClick={onRetry}>
            {S.dataState.retry}
          </Button>
        )}
      </EmptyState>
    );
  }

  if (empty) {
    return (
      <EmptyState
        title={emptyTitle ?? S.dataState.emptyTitle}
        image={<Icon name={emptyIcon} />}
      >
        {emptyBody && <p>{emptyBody}</p>}
      </EmptyState>
    );
  }

  return <>{children}</>;
}
