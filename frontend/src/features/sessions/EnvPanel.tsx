import { ChangeEvent, FormEvent, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  Chip,
  Form,
  Icon,
  Link,
  MainTable,
  Select,
  useNotify,
} from '@canonical/react-components';
import type { MainTableProps } from '@canonical/react-components';
import { DataState } from '../../components/DataState';
import { getEnvLogs, getEnvStatus, launchEnv, listEnvSpecs, relaunchEnv } from '../../lib/api/env';
import { isApiError } from '../../lib/api/errors';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys } from '../../lib/routes';
import type { EnvironmentState } from '../../lib/types';
import { S } from '../../strings/catalogue';

type Row = NonNullable<MainTableProps['rows']>[number];
type ChipAppearance = 'caution' | 'information' | 'negative' | 'positive';

const DEFAULT_ENVIRONMENT = 'dev';

function statusAppearance(status: string): ChipAppearance {
  switch (status) {
    case 'ready':
      return 'positive';
    case 'degraded':
      return 'caution';
    default:
      return 'information';
  }
}

function healthAppearance(health: string): ChipAppearance {
  return health === 'healthy' ? 'positive' : 'caution';
}

function statusLabel(status: string): string {
  switch (status) {
    case 'ready':
      return S.env.status.ready;
    case 'degraded':
      return S.env.status.degraded;
    default:
      return `${S.env.status.unknown} (${status})`;
  }
}

function healthLabel(health: string): string {
  return health === 'healthy' ? S.env.health.healthy : S.env.health.degraded;
}

interface EnvPanelProps {
  sessionId: string;
}

export function EnvPanel({ sessionId }: EnvPanelProps) {
  const [draft, setDraft] = useState('');
  const [environmentName, setEnvironmentName] = useState(DEFAULT_ENVIRONMENT);
  const [logComponent, setLogComponent] = useState<string | null>(null);
  const queryClient = useQueryClient();
  const notify = useNotify();

  const specsQuery = useQuery({
    queryKey: queryKeys.env.specs(),
    queryFn: listEnvSpecs,
    retry: false,
  });

  const specs = specsQuery.data ?? [];

  const statusQuery = useQuery({
    queryKey: queryKeys.env.status(sessionId, environmentName),
    queryFn: () => getEnvStatus(sessionId, environmentName),
    retry: false,
  });

  const logsQuery = useQuery({
    queryKey: queryKeys.env.logs(sessionId, environmentName, logComponent ?? ''),
    queryFn: () => getEnvLogs(sessionId, environmentName, logComponent ?? ''),
    enabled: !!logComponent,
    retry: false,
  });

  const refresh = () => {
    queryClient.invalidateQueries({ queryKey: queryKeys.env.status(sessionId, environmentName) });
    if (logComponent) {
      queryClient.invalidateQueries({
        queryKey: queryKeys.env.logs(sessionId, environmentName, logComponent),
      });
    }
  };

  const launchMutation = useMutation({
    mutationFn: () => launchEnv(sessionId, environmentName),
    onSuccess: (result) => {
      refresh();
      const failed = result.components.filter((component) => !component.success);
      if (result.success) {
        notify.success(S.env.launchResult.success);
        return;
      }
      notify.failure(
        S.env.launchResult.failure,
        new Error(failed.map((component) => `${component.name}: ${component.message}`).join(', ')),
      );
    },
    onError: (error) => notify.failure(S.env.launchResult.failure, error, getErrorMessage(error)),
  });

  const relaunchMutation = useMutation({
    mutationFn: () => relaunchEnv(sessionId, environmentName),
    onSuccess: (diff) => {
      refresh();
      if (diff.components_to_restart.length === 0) {
        notify.info(S.env.relaunchResult.unchanged);
        return;
      }
      notify.success(S.env.relaunchResult.success);
    },
    onError: (error) => notify.failure(S.env.relaunchResult.success, error, getErrorMessage(error)),
  });

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setLogComponent(null);
    setEnvironmentName(draft);
  };

  const noSpecs = !specsQuery.isLoading && !specsQuery.error && specs.length === 0;

  const state: EnvironmentState | undefined = statusQuery.data;
  const specMissing = isApiError(statusQuery.error) && statusQuery.error.status === 404;

  const componentRows: Row[] = (state?.components ?? []).map((component) => ({
    columns: [
      { content: component.name },
      {
        content: (
          <Chip
            value={statusLabel(component.status)}
            appearance={statusAppearance(component.status)}
            isReadOnly
            isDense
          />
        ),
      },
      {
        content: (
          <Chip
            value={healthLabel(component.health)}
            appearance={healthAppearance(component.health)}
            isReadOnly
            isDense
          />
        ),
      },
      {
        content: (
          <Button
            appearance="link"
            onClick={() =>
              setLogComponent(logComponent === component.name ? null : component.name)
            }
          >
            {S.env.logs.view}
          </Button>
        ),
      },
    ],
  }));

  const exposureRows: Row[] = (state?.exposures ?? []).map((exposure) => ({
    columns: [
      { content: exposure.name },
      { content: exposure.as_type },
      {
        content: (
          <Link href={exposure.url} target="_blank" rel="noreferrer noopener">
            {exposure.url}
          </Link>
        ),
      },
    ],
  }));

  return (
    <div className="u-no-margin--bottom">
      {noSpecs ? (
        <DataState
          loading={false}
          error={null}
          empty
          emptyIcon="help"
          emptyTitle={S.env.specMissingTitle}
          emptyBody={S.env.specMissingBody}
        >
          <p />
        </DataState>
      ) : (
        <Form inline onSubmit={handleSubmit}>
          <Select
            id="env-name"
            label={S.env.environmentLabel}
            help={S.env.environmentHelp}
            options={specs.map((name) => ({ value: name, label: name }))}
            value={draft || environmentName}
            onChange={(event: ChangeEvent<HTMLSelectElement>) => setDraft(event.target.value)}
          />
          <Button appearance="positive" type="submit">
            <Icon name="search" />
            {S.env.refresh}
          </Button>
        </Form>
      )}

      <div className="u-flex u-justify-space-between u-align--center u-margin--bottom">
        {state ? (
          <Chip
            value={`${state.name}: ${statusLabel(state.status)}`}
            appearance={statusAppearance(state.status)}
            isReadOnly
          />
        ) : (
          <span />
        )}
        <div className="u-flex">
          <Button
            appearance="positive"
            disabled={launchMutation.isPending}
            onClick={() => launchMutation.mutate()}
          >
            <Icon name="play" />
            {S.env.launch}
          </Button>
          <Button
            className="u-no-margin--bottom"
            disabled={relaunchMutation.isPending}
            onClick={() => relaunchMutation.mutate()}
          >
            <Icon name="restart" />
            {S.env.relaunch}
          </Button>
        </div>
      </div>

      {noSpecs ? null : specMissing ? (
        <DataState
          loading={false}
          error={null}
          empty
          emptyIcon="help"
          emptyTitle={S.env.specMissingTitle}
          emptyBody={S.env.specMissingBody}
        >
          <p />
        </DataState>
      ) : (
        <DataState
          loading={statusQuery.isLoading}
          error={statusQuery.error ? getErrorMessage(statusQuery.error) : null}
          empty={!statusQuery.isLoading && !statusQuery.error && !state?.components.length}
          emptyTitle={S.env.columns.component}
          emptyBody={S.env.environmentHelp}
          onRetry={() => statusQuery.refetch()}
        >
          <MainTable
            headers={[
              { content: S.env.columns.component },
              { content: S.env.columns.status },
              { content: S.env.columns.health },
              { content: S.env.columns.logs },
            ]}
            rows={componentRows}
            responsive
          />
        </DataState>
      )}

      {logComponent && (
        <div className="u-margin--bottom">
          <h4 className="p-heading--5">
            {`${S.env.logs.title}: ${logComponent}`}
          </h4>
          <DataState
            loading={logsQuery.isLoading}
            error={logsQuery.error ? getErrorMessage(logsQuery.error) : null}
            empty={!logsQuery.isLoading && !logsQuery.error && !logsQuery.data?.lines.length}
            emptyTitle={S.env.logs.title}
            emptyBody={S.env.logs.emptyBody}
            onRetry={() => logsQuery.refetch()}
          >
            <pre className="p-code-snippet__block">
              {(logsQuery.data?.lines ?? []).join('\n')}
            </pre>
          </DataState>
        </div>
      )}

      {!!state?.exposures.length && (
        <div className="u-margin--bottom">
          <h4 className="p-heading--5">{S.env.exposures.title}</h4>
          <MainTable
            headers={[
              { content: S.env.exposures.title },
              { content: S.env.columns.status },
              { content: S.env.environmentLabel },
            ]}
            rows={exposureRows}
            responsive
          />
        </div>
      )}
    </div>
  );
}
