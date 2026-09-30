import { useState, type FormEvent } from 'react';
import { Button, Form, Input } from '@canonical/react-components';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { createOrg } from '../../lib/api/projects';
import { getErrorMessage } from '../../lib/api/errors';
import { queryKeys } from '../../lib/routes';
import { S } from '../../strings/catalogue';

function slugOf(name: string): string {
  return name
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 48);
}

/**
 * A new account belongs to no organisation, so it has nothing to show. This is
 * the way out, rather than an empty list with no explanation.
 */
export function NoOrgState() {
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [slugEdited, setSlugEdited] = useState(false);
  const queryClient = useQueryClient();

  const create = useMutation({
    mutationFn: () => createOrg({ name, slug: slug || slugOf(name) }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: queryKeys.orgs.all() }),
  });

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (name.trim().length === 0) return;
    create.mutate();
  };

  return (
    <div className="app-empty-org" data-testid="no-org">
      <h1 className="p-heading--3">{S.projectsFirstOrg.firstOrgTitle}</h1>
      <p className="u-text--muted">{S.projectsFirstOrg.firstOrgBody}</p>
      <Form onSubmit={handleSubmit}>
        {create.isError ? (
          <p className="p-form-validation__message" role="alert">
            {getErrorMessage(create.error)}
          </p>
        ) : null}
        <Input
          id="org-name"
          type="text"
          label={S.projectsFirstOrg.orgName}
          value={name}
          onChange={(event) => {
            setName(event.target.value);
            if (!slugEdited) setSlug(slugOf(event.target.value));
          }}
        />
        <Input
          id="org-slug"
          type="text"
          label={S.projectsFirstOrg.orgSlug}
          value={slug}
          onChange={(event) => {
            setSlugEdited(true);
            setSlug(event.target.value);
          }}
        />
        <Button
          type="submit"
          appearance="positive"
          disabled={name.trim().length === 0 || create.isPending}
        >
          {S.projectsFirstOrg.createOrg}
        </Button>
      </Form>
    </div>
  );
}
