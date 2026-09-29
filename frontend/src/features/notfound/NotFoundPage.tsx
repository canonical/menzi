import { Link } from 'react-router-dom';
import { Button, EmptyState, Icon } from '@canonical/react-components';
import { S } from '../../strings/catalogue';

export function NotFoundPage() {
  return (
    <EmptyState title={S.notFound.title} image={<Icon name="search" />}>
      <p>{S.notFound.body}</p>
      <Link to="/projects">
        <Button appearance="positive">{S.notFound.goToProjects}</Button>
      </Link>
    </EmptyState>
  );
}