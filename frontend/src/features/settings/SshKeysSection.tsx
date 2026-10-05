import { FormEvent, useState } from 'react';
import {
  Button,
  Form,
  Input,
  MainTable,
  Modal,
  Spinner,
  Textarea,
  useNotify,
} from '@canonical/react-components';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  createUserSshKey,
  deleteUserSshKey,
  generateUserSshKey,
  listUserSshKeys,
} from '../../lib/api/sshKeys';
import { getErrorMessage } from '../../lib/api/errors';
import { S } from '../../strings/catalogue';

export function SshKeysSection() {
  const notify = useNotify();
  const queryClient = useQueryClient();
  const [addOpen, setAddOpen] = useState(false);
  const [generateOpen, setGenerateOpen] = useState(false);
  const [label, setLabel] = useState('');
  const [generateLabel, setGenerateLabel] = useState('');
  const [publicKey, setPublicKey] = useState('');
  const [privateKey, setPrivateKey] = useState('');
  const [copiedKeyId, setCopiedKeyId] = useState<string | null>(null);

  const query = useQuery({
    queryKey: ['ssh-keys'],
    queryFn: listUserSshKeys,
    retry: false,
  });

  const invalidate = () => queryClient.invalidateQueries({ queryKey: ['ssh-keys'] });

  const create = useMutation({
    mutationFn: () =>
      createUserSshKey({
        label,
        public_key: publicKey,
        private_key: privateKey,
      }),
    onSuccess: () => {
      setAddOpen(false);
      setLabel('');
      setPublicKey('');
      setPrivateKey('');
      invalidate();
      notify.success(S.settings.sshKeys.added);
    },
    onError: (error) => notify.failure(S.settings.sshKeys.failed, error, getErrorMessage(error)),
  });

  const generate = useMutation({
    mutationFn: () => generateUserSshKey({ label: generateLabel.trim() }),
    onSuccess: () => {
      setGenerateOpen(false);
      setGenerateLabel('');
      invalidate();
      notify.success(S.settings.sshKeys.generated);
    },
    onError: (error) => notify.failure(S.settings.sshKeys.failed, error, getErrorMessage(error)),
  });

  const remove = useMutation({
    mutationFn: (id: string) => deleteUserSshKey(id),
    onSuccess: () => {
      invalidate();
      notify.success(S.settings.sshKeys.removed);
    },
    onError: (error) => notify.failure(S.settings.sshKeys.failed, error, getErrorMessage(error)),
  });

  const submit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    create.mutate();
  };

  const copyPublicKey = async (keyId: string, value: string) => {
    try {
      await navigator.clipboard.writeText(value);
      setCopiedKeyId(keyId);
      window.setTimeout(() => {
        setCopiedKeyId((current) => (current === keyId ? null : current));
      }, 550);
      notify.success(S.settings.sshKeys.copied);
    } catch {
      notify.failure(S.settings.sshKeys.failed);
    }
  };

  const shortPublicKey = (value: string) => {
    const trimmed = value.trim();
    if (trimmed.length <= 48) return trimmed;
    return `${trimmed.slice(0, 48)}...`;
  };

  return (
    <div>
      <h3 className="p-heading--4">{S.settings.sshKeys.title}</h3>
      <div className="u-margin--bottom">
        <Button appearance="positive" type="button" onClick={() => setAddOpen(true)}>
          {S.settings.sshKeys.add}
        </Button>
        <Button
          type="button"
          onClick={() => setGenerateOpen(true)}
          disabled={generate.isPending}
        >
          {generate.isPending ? <Spinner text={S.dataState.loading} /> : S.settings.sshKeys.generate}
        </Button>
      </div>
      {query.isLoading ? (
        <Spinner text={S.dataState.loading} />
      ) : query.error ? (
        <p>{getErrorMessage(query.error)}</p>
      ) : (query.data?.length ?? 0) === 0 ? (
        <div className="p-card">
          <div className="p-card__content">
            <p>{S.settings.sshKeys.empty}</p>
          </div>
        </div>
      ) : (
        <MainTable
          headers={[
            { content: S.settings.sshKeys.label },
            { content: S.settings.sshKeys.publicKey },
            { content: S.projects.columns.actions },
          ]}
          rows={(query.data ?? []).map((key) => ({
            columns: [
              { content: key.label },
              { content: shortPublicKey(key.public_key) },
              {
                content: (
                  <div className="u-flex">
                    <Button
                      className={`u-no-margin--bottom app-ssh-copy-button${copiedKeyId === key.id ? ' is-copied' : ''}`}
                      onClick={() => copyPublicKey(key.id, key.public_key)}
                    >
                      {S.settings.sshKeys.copy}
                    </Button>
                    <Button className="u-no-margin--bottom" onClick={() => remove.mutate(key.id)}>
                      {S.settings.sshKeys.remove}
                    </Button>
                  </div>
                ),
              },
            ],
          }))}
          responsive
        />
      )}
      {addOpen ? (
        <Modal
          title={S.settings.sshKeys.add}
          close={() => setAddOpen(false)}
          buttonRow={
            <>
              <Button className="u-no-margin--bottom" onClick={() => setAddOpen(false)}>
                {S.settings.sshKeys.cancel}
              </Button>
              <Button
                appearance="positive"
                className="u-no-margin--bottom"
                disabled={create.isPending}
                onClick={() => {
                  if (!create.isPending) create.mutate();
                }}
              >
                {create.isPending ? <Spinner text={S.dataState.loading} /> : S.settings.sshKeys.add}
              </Button>
            </>
          }
        >
          <Form onSubmit={submit}>
            <Input
              label={S.settings.sshKeys.label}
              type="text"
              value={label}
              onChange={(event) => setLabel(event.target.value)}
            />
            <Textarea
              label={S.settings.sshKeys.publicKey}
              value={publicKey}
              onChange={(event) => setPublicKey(event.target.value)}
              rows={4}
            />
            <Textarea
              label={S.settings.sshKeys.privateKey}
              value={privateKey}
              onChange={(event) => setPrivateKey(event.target.value)}
              rows={8}
            />
          </Form>
        </Modal>
      ) : null}
      {generateOpen ? (
        <Modal
          title={S.settings.sshKeys.generate}
          close={() => setGenerateOpen(false)}
          buttonRow={
            <>
              <Button className="u-no-margin--bottom" onClick={() => setGenerateOpen(false)}>
                {S.settings.sshKeys.cancel}
              </Button>
              <Button
                appearance="positive"
                className="u-no-margin--bottom"
                disabled={generate.isPending || !generateLabel.trim()}
                onClick={() => {
                  if (!generate.isPending && generateLabel.trim()) generate.mutate();
                }}
              >
                {generate.isPending ? <Spinner text={S.dataState.loading} /> : S.settings.sshKeys.generate}
              </Button>
            </>
          }
        >
          <Form
            onSubmit={(event) => {
              event.preventDefault();
              if (!generate.isPending && generateLabel.trim()) generate.mutate();
            }}
          >
            <Input
              label={S.settings.sshKeys.label}
              type="text"
              value={generateLabel}
              onChange={(event) => setGenerateLabel(event.target.value)}
            />
          </Form>
        </Modal>
      ) : null}
    </div>
  );
}
