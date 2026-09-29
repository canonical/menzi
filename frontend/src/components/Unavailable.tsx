import { EmptyState, Icon } from '@canonical/react-components';
import { S } from '../strings/catalogue';

export function Unavailable({ icon = 'help' }: { icon?: string }) {
  return (
    <EmptyState title={S.unavailable.title} image={<Icon name={icon} />}>
      <p>{S.unavailable.body}</p>
    </EmptyState>
  );
}