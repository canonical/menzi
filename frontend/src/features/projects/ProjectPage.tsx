import { Link, useParams } from 'react-router-dom';
import { useQuery } from '@tanstack/react-query';
import { Button, Icon, Spinner } from '@canonical/react-components';
import { DataState } from '../../components/DataState';
import { WorkspaceCard } from '../workspaces/WorkspaceCard';
import { getProject } from '../../lib/api/projects';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys, routes } from '../../lib/routes';
import { formatDateTime } from '../../lib/format/time';
import { useAuthStore } from '../../stores/auth';
import { S } from '../../strings/catalogue';

export function ProjectPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const user = useAuthStore((state) => state.user);
  const projectQuery = useQuery({
    queryKey: queryKeys.projects.detail(projectId ?? ''),
    queryFn: () => getProject(projectId ?? ''),
    enabled: !!projectId,
    retry: false,
  });

  const project = projectQuery.data;

  if (projectQuery.isLoading) {
    return (
      <div className="u-align--center u-vertically-center">
        <Spinner text={S.dataState.loading} />
      </div>
    );
  }

  return (
    <div>
      <DataState
        loading={false}
        error={projectQuery.error ? getErrorMessage(projectQuery.error) : null}
        empty={!projectQuery.error && !project}
        emptyIcon="code"
        emptyTitle={S.projects.emptyTitle}
        emptyBody={S.projects.emptyBody}
        onRetry={() => projectQuery.refetch()}
      >
        {project ? (
          <div>
            <h1 className="p-heading--2">{project.name}</h1>
            {project.description ? <p>{project.description}</p> : null}

            <div className="app-page-header__actions u-margin--bottom">
              <Button
                appearance="positive"
                element={Link}
                to={routes.projects.previews(project.id)}
              >
                <Icon name="code" />
                {S.projects.detail.previews}
              </Button>
              <Button
                className="u-no-margin--bottom"
                element={Link}
                to={routes.projects.design(project.id)}
              >
                <Icon name="file-blank" />
                {S.projects.detail.design}
              </Button>
              <Button
                className="u-no-margin--bottom"
                element={Link}
                to={routes.projects.developmentScripts(project.id)}
              >
                <Icon name="code" />
                {S.sections.developmentScripts}
              </Button>
            </div>

            <WorkspaceCard projectId={project.id} userId={user?.id} />

            <div className="p-card">
              <div className="p-card__content">
                <dl className="p-definition-list">
                  <dt className="p-definition-list__term">{S.projects.detail.slug}</dt>
                  <dd className="p-definition-list__definition">{project.slug}</dd>
                  <dt className="p-definition-list__term">{S.projects.detail.repositoryUrl}</dt>
                  <dd className="p-definition-list__definition">{project.repository_url ?? '—'}</dd>
                  <dt className="p-definition-list__term">{S.projects.detail.created}</dt>
                  <dd className="p-definition-list__definition">
                    {formatDateTime(project.created_at)}
                  </dd>
                  <dt className="p-definition-list__term">{S.projects.detail.updated}</dt>
                  <dd className="p-definition-list__definition">
                    {formatDateTime(project.updated_at)}
                  </dd>
                </dl>
              </div>
            </div>
          </div>
        ) : (
          <p />
        )}
      </DataState>
    </div>
  );
}
