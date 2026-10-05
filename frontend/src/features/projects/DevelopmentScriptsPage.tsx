import { FormEvent, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Button, Form, Input, MainTable, Spinner, Textarea } from '@canonical/react-components';
import { useParams } from 'react-router-dom';
import {
  createDevelopmentScript,
  deleteDevelopmentScript,
  listDevelopmentScripts,
  updateDevelopmentScript,
} from '../../lib/api/developmentScripts';
import { getErrorMessage } from '../../lib/api/errors';
import { S } from '../../strings/catalogue';

export function DevelopmentScriptsPage() {
  const { projectId } = useParams<{ projectId: string }>();
  const queryClient = useQueryClient();
  const [editing, setEditing] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [relativePath, setRelativePath] = useState('');
  const [body, setBody] = useState('');

  const query = useQuery({
    queryKey: ['development-scripts', projectId],
    queryFn: () => listDevelopmentScripts(projectId ?? ''),
    enabled: !!projectId,
    retry: false,
  });

  const refresh = () => queryClient.invalidateQueries({ queryKey: ['development-scripts', projectId] });

  const create = useMutation({
    mutationFn: () =>
      createDevelopmentScript(projectId ?? '', {
        name,
        slug,
        relative_path: relativePath || undefined,
        body,
      }),
    onSuccess: () => {
      setName('');
      setSlug('');
      setRelativePath('');
      setBody('');
      refresh();
    },
  });

  const update = useMutation({
    mutationFn: (id: string) =>
      updateDevelopmentScript(projectId ?? '', id, {
        name,
        slug,
        relative_path: relativePath || undefined,
        body,
      }),
    onSuccess: () => {
      setEditing(null);
      setName('');
      setSlug('');
      setRelativePath('');
      setBody('');
      refresh();
    },
  });

  const remove = useMutation({
    mutationFn: (id: string) => deleteDevelopmentScript(projectId ?? '', id),
    onSuccess: () => refresh(),
  });

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (editing) {
      update.mutate(editing);
      return;
    }
    create.mutate();
  };

  return (
    <div>
      <h1 className="p-heading--2">{S.developmentScripts.title}</h1>
      <p>{S.developmentScripts.description}</p>
      <div className="p-card u-margin--bottom">
        <div className="p-card__content">
          <Form onSubmit={submit}>
            <Input label={S.developmentScripts.name} type="text" value={name} onChange={(e) => setName(e.target.value)} />
            <Input label={S.developmentScripts.slug} type="text" value={slug} onChange={(e) => setSlug(e.target.value)} />
            <Input label={S.developmentScripts.path} type="text" value={relativePath} onChange={(e) => setRelativePath(e.target.value)} />
            <Textarea label={S.developmentScripts.body} value={body} onChange={(e) => setBody(e.target.value)} rows={10} />
            <Button appearance="positive" type="submit" disabled={create.isPending || update.isPending}>
              {create.isPending || update.isPending ? <Spinner text={S.dataState.loading} /> : S.developmentScripts.save}
            </Button>
            {editing ? (
              <Button
                type="button"
                onClick={() => {
                  setEditing(null);
                  setName('');
                  setSlug('');
                  setRelativePath('');
                  setBody('');
                }}
              >
                {S.developmentScripts.cancel}
              </Button>
            ) : null}
          </Form>
        </div>
      </div>
      {query.isLoading ? (
        <Spinner text={S.dataState.loading} />
      ) : query.error ? (
        <p>{getErrorMessage(query.error)}</p>
      ) : (query.data?.length ?? 0) === 0 ? (
        <div className="p-card">
          <div className="p-card__content">
            <h3 className="p-heading--5">{S.developmentScripts.emptyTitle}</h3>
            <p>{S.developmentScripts.emptyBody}</p>
          </div>
        </div>
      ) : (
        <MainTable
          headers={[
            { content: S.developmentScripts.name },
            { content: S.developmentScripts.slug },
            { content: S.developmentScripts.path },
            { content: S.developmentScripts.source },
            { content: S.projects.columns.actions },
          ]}
          rows={(query.data ?? []).map((script) => ({
            columns: [
              { content: script.name },
              { content: script.slug },
              { content: script.relative_path ?? '—' },
              { content: script.source },
              {
                content: (
                  <>
                    <Button
                      appearance="link"
                      onClick={() => {
                        setEditing(script.id);
                        setName(script.name);
                        setSlug(script.slug);
                        setRelativePath(script.relative_path ?? '');
                        setBody(script.body);
                      }}
                    >
                      {S.projects.actions.open}
                    </Button>
                    <Button appearance="link" onClick={() => remove.mutate(script.id)}>
                      {S.developmentScripts.cancel}
                    </Button>
                  </>
                ),
              },
            ],
          }))}
          responsive
        />
      )}
    </div>
  );
}
