import { FormEvent, useState } from 'react';
import { Link } from 'react-router-dom';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Button,
  ContextualMenu,
  Form,
  Icon,
  Input,
  MainTable,
  Modal,
  Select,
  Spinner,
  useNotify,
} from '@canonical/react-components';
import type { MainTableProps } from '@canonical/react-components';
import { DataState } from '../../components/DataState';
import { createProject, listOrgs, listProjects } from '../../lib/api/projects';
import { getErrorMessage, isApiError } from '../../lib/api/errors';
import { queryKeys, routes } from '../../lib/routes';
import { formatDateTime } from '../../lib/format/time';
import { S } from '../../strings/catalogue';

type Row = NonNullable<MainTableProps['rows']>[number];

const SLUG_PATTERN = /^[a-z0-9-]+$/;

function slugify(value: string): string {
  return value
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
}

export function ProjectsPage() {
  const [search, setSearch] = useState('');
  const [orgId, setOrgId] = useState('');
  const [createOpen, setCreateOpen] = useState(false);
  const queryClient = useQueryClient();
  const notify = useNotify();

  const projectsQuery = useQuery({
    queryKey: queryKeys.projects.filtered({ orgId, search }),
    queryFn: () => listProjects({ orgId: orgId || undefined, search: search || undefined }),
  });

  const orgsQuery = useQuery({
    queryKey: queryKeys.orgs.all(),
    queryFn: listOrgs,
  });

  const createMutation = useMutation({
    mutationFn: createProject,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.projects.all() });
      setCreateOpen(false);
      notify.success(S.projects.toasts.created);
    },
    onError: (error) =>
      notify.failure(S.projects.toasts.failed, error, getErrorMessage(error)),
  });

  const orgNames = new Map((orgsQuery.data ?? []).map((org) => [org.id, org.name]));

  const rows: Row[] = (projectsQuery.data ?? []).map((project) => ({
    columns: [
      {
        content: (
          <Link to={routes.projects.project(project.id)}>{project.name}</Link>
        ),
      },
      { content: project.slug },
      { content: orgNames.get(project.org_id) ?? project.org_id },
      { content: formatDateTime(project.updated_at) },
      {
        content: (
          <ContextualMenu
            hasToggleIcon
            position="right"
            toggleLabel={S.projects.columns.actions}
            toggleAppearance="base"
            links={[
              {
                children: S.projects.actions.open,
                element: Link,
                to: routes.projects.project(project.id),
              },
              {
                children: S.projects.actions.previews,
                element: Link,
                to: routes.projects.previews(project.id),
              },
            ]}
          />
        ),
      },
    ],
  }));

  return (
    <div className="app-section">
      <div className="app-page-header">
        <div>
          <h1 className="p-heading--2">{S.projects.title}</h1>
          <p className="u-text--muted u-no-margin--bottom">{S.projects.description}</p>
        </div>
        <div className="app-page-header__actions">
          <Button appearance="positive" onClick={() => setCreateOpen(true)}>
            <Icon name="plus" />
            {S.projects.create}
          </Button>
        </div>
      </div>

      <div className="app-toolbar">
        <div className="app-toolbar__field">
          <Input
            id="projects-search"
            type="search"
            label={S.projects.searchLabel}
            placeholder={S.projects.searchPlaceholder}
            value={search}
            onChange={(event) => setSearch(event.target.value)}
          />
        </div>
        <div className="app-toolbar__field">
          <Select
            id="projects-org"
            label={S.projects.filterLabel}
            options={[
              { value: '', label: S.projects.filterAll },
              ...(orgsQuery.data ?? []).map((org) => ({ value: org.id, label: org.name })),
            ]}
            value={orgId}
            onChange={(event) => setOrgId(event.target.value)}
          />
        </div>
      </div>

      <DataState
        loading={projectsQuery.isLoading}
        error={projectsQuery.error ? getErrorMessage(projectsQuery.error) : null}
        empty={!projectsQuery.isLoading && !projectsQuery.error && !projectsQuery.data?.length}
        emptyIcon="code"
        emptyTitle={S.projects.emptyTitle}
        emptyBody={S.projects.emptyBody}
        onRetry={() => projectsQuery.refetch()}
      >
        <MainTable
          headers={[
            { content: S.projects.columns.name },
            { content: S.projects.columns.slug },
            { content: S.projects.columns.org },
            { content: S.projects.columns.updated },
            { content: S.projects.columns.actions },
          ]}
          rows={rows}
          responsive
        />
      </DataState>

      {createOpen && (
        <CreateProjectModal
          orgs={orgsQuery.data ?? []}
          pending={createMutation.isPending}
          error={
            createMutation.error
              ? isApiError(createMutation.error) && createMutation.error.status === 409
                ? S.projects.errors.duplicateSlug
                : getErrorMessage(createMutation.error)
              : null
          }
          onClose={() => {
            setCreateOpen(false);
            createMutation.reset();
          }}
          onSubmit={(input) => createMutation.mutate(input)}
        />
      )}
    </div>
  );
}

interface CreateProjectModalProps {
  orgs: { id: string; name: string }[];
  pending: boolean;
  error: string | null;
  onClose: () => void;
  onSubmit: (input: {
    orgId: string;
    name: string;
    slug: string;
    description?: string;
  }) => void;
}

function CreateProjectModal({
  orgs,
  pending,
  error,
  onClose,
  onSubmit,
}: CreateProjectModalProps) {
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [slugTouched, setSlugTouched] = useState(false);
  const [description, setDescription] = useState('');
  const [org, setOrg] = useState('');
  const [fieldErrors, setFieldErrors] = useState<{
    name: string | null;
    slug: string | null;
    org: string | null;
  }>({ name: null, slug: null, org: null });

  const handleName = (value: string) => {
    setName(value);
    if (!slugTouched) setSlug(slugify(value));
  };

  const submit = () => {
    const nextName = name.trim() ? null : S.projects.errors.nameRequired;
    const nextSlug = !slug.trim()
      ? S.projects.errors.slugRequired
      : SLUG_PATTERN.test(slug)
        ? null
        : S.projects.errors.slugInvalid;
    const nextOrg = org ? null : S.projects.errors.orgRequired;
    setFieldErrors({ name: nextName, slug: nextSlug, org: nextOrg });
    if (nextName || nextSlug || nextOrg) return;
    onSubmit({
      orgId: org,
      name: name.trim(),
      slug: slug.trim(),
      description: description.trim() || undefined,
    });
  };

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    submit();
  };

  return (
    <Modal
      title={S.projects.createTitle}
      close={onClose}
      buttonRow={
        <>
          <Button className="u-no-margin--bottom" onClick={onClose}>
            Cancel
          </Button>
          <Button
            appearance="positive"
            className="u-no-margin--bottom"
            disabled={pending}
            onClick={submit}
          >
            {pending ? <Spinner text={S.dataState.loading} /> : S.projects.create}
          </Button>
        </>
      }
    >
      <Form onSubmit={handleSubmit}>
        <Input
          id="new-project-name"
          label={S.projects.nameLabel}
          type="text"
          required
          value={name}
          error={fieldErrors.name}
          onChange={(event) => handleName(event.target.value)}
        />
        <Input
          id="new-project-slug"
          label={S.projects.slugLabel}
          type="text"
          required
          value={slug}
          help={S.projects.errors.slugInvalid}
          error={fieldErrors.slug}
          onChange={(event) => {
            setSlugTouched(true);
            setSlug(event.target.value);
          }}
        />
        <Select
          id="new-project-org"
          label={S.projects.orgLabel}
          options={orgs.map((item) => ({ value: item.id, label: item.name }))}
          value={org}
          error={fieldErrors.org}
          onChange={(event) => setOrg(event.target.value)}
        />
        <Input
          id="new-project-description"
          label={S.projects.descriptionLabel}
          type="text"
          value={description}
          onChange={(event) => setDescription(event.target.value)}
        />
        {error ? (
          <p className="p-form-validation__message" role="alert">
            {error}
          </p>
        ) : null}
      </Form>
    </Modal>
  );
}
