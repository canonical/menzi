import { useTranscript } from '../../hooks/useTranscript';
import { DataState } from '../../components/DataState';
import { Chip, MainTable, Spinner } from '@canonical/react-components';
import type { MainTableProps } from '@canonical/react-components';
import type { TunnelMessage } from '../../lib/types';
import { S } from '../../strings/catalogue';

type Row = NonNullable<MainTableProps['rows']>[number];

type ChipAppearance = 'caution' | 'information' | 'negative' | 'positive';

const STATUS_LABELS: Record<string, string> = {
  idle: S.transcript.status.idle,
  loading: S.transcript.status.loading,
  live: S.transcript.status.live,
  reconnecting: S.transcript.status.reconnecting,
  error: S.transcript.status.reconnecting,
};

const STATUS_APPEARANCE: Record<string, ChipAppearance> = {
  idle: 'information',
  loading: 'information',
  live: 'positive',
  reconnecting: 'caution',
  error: 'negative',
};

function summarise(payload: Record<string, unknown>): string {
  const text = JSON.stringify(payload);
  if (text === undefined || text === '{}') return '-';
  return text.length > 160 ? `${text.slice(0, 157)}...` : text;
}

function toRows(events: TunnelMessage[]): Row[] {
  return events.map((event) => ({
    columns: [
      { content: String(event.sequence) },
      { content: event.message_type },
      { content: <code className="u-no-margin--bottom">{summarise(event.payload)}</code> },
    ],
  }));
}

export function TranscriptView({ sessionId }: { sessionId: string }) {
  const { events, status, error, loaded, reconnect } = useTranscript(sessionId, {
    enabled: true,
  });

  const rows = toRows(events);

  return (
    <div>
      <div className="u-flex u-justify-space-between u-align--center u-margin--bottom">
        <h2 className="p-heading--3">{S.transcript.title}</h2>
        <Chip
          value={STATUS_LABELS[status] ?? status}
          appearance={STATUS_APPEARANCE[status] ?? 'information'}
          isReadOnly
        />
      </div>

      {error ? (
        <div className="u-margin--bottom">
          <DataState
            loading={false}
            error={error}
            empty={false}
            onRetry={reconnect}
          >
            <p />
          </DataState>
        </div>
      ) : null}

      {status === 'reconnecting' && !error ? (
        <div className="u-flex u-align--center u-margin--bottom">
          <Spinner text={S.transcript.status.reconnecting} />
        </div>
      ) : null}

      <DataState
        loading={!loaded && events.length === 0}
        error={null}
        empty={loaded && events.length === 0}
        emptyTitle={S.transcript.emptyTitle}
        emptyBody={S.transcript.emptyBody}
      >
        <MainTable
          headers={[
            { content: S.transcript.columns.sequence },
            { content: S.transcript.columns.type },
            { content: S.transcript.columns.payload },
          ]}
          rows={rows}
          responsive
        />
      </DataState>
    </div>
  );
}
