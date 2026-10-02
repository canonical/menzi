import { useEffect, useId, useRef, useState } from 'react';
import { Button, CheckboxInput, Form, RadioInput, Textarea } from '@canonical/react-components';
import type { FormAnswer, QuestionField, QuestionForm } from '../../../lib/api/forms';
import { getErrorMessage } from '../../../lib/api/errors';
import { S } from '../../../strings/catalogue';

interface Draft {
  choices: string[];
  custom: boolean;
  text: string;
}

export function QuestionCard({ request, onSubmit, onDismiss }: {
  request: QuestionForm;
  onSubmit: (answer: FormAnswer) => Promise<void>;
  onDismiss?: () => Promise<void>;
}) {
  const id = useId();
  const storageKey = `menzi:question:${request.sessionID}:${request.id}`;
  const [drafts, setDrafts] = useState<Record<string, Draft>>(() => {
    try {
      const value = JSON.parse(sessionStorage.getItem(storageKey) ?? '{}') as Record<string, Draft>;
      return Object.fromEntries(Object.entries(value).filter(([, draft]) => draft && Array.isArray(draft.choices) && typeof draft.custom === 'boolean' && typeof draft.text === 'string'));
    } catch { return {}; }
  });
  const [submitted, setSubmitted] = useState<FormAnswer | null>(null);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sendingRef = useRef(false);
  useEffect(() => {
    try {
      if (submitted || (request.state && request.state.status !== 'pending')) sessionStorage.removeItem(storageKey);
      else sessionStorage.setItem(storageKey, JSON.stringify(drafts));
    } catch { return; }
  }, [drafts, storageKey, submitted, request.state]);
  const draftFor = (key: string): Draft => drafts[key] ?? { choices: [], custom: false, text: '' };
  const update = (key: string, value: Partial<Draft>) => setDrafts((previous) => ({
    ...previous, [key]: { ...draftFor(key), ...value },
  }));
  const answer: FormAnswer = {};
  request.fields.forEach((field) => {
    const draft = draftFor(field.key);
    answer[field.key] = field.type === 'multiselect'
      ? [...draft.choices, ...(draft.custom && draft.text.trim() ? [draft.text.trim()] : [])]
      : field.options?.length && !draft.custom ? draft.choices[0] ?? '' : draft.text.trim();
  });
  const fields = request.fields.filter((field) => !field.hidden && (field.when ?? []).every((condition) => {
    const same = JSON.stringify(answer[condition.key]) === JSON.stringify(condition.value);
    return condition.op === 'eq' ? same : !same;
  }));
  const valid = (field: QuestionField) => {
    const value = answer[field.key];
    const draft = draftFor(field.key);
    if (draft.custom && !draft.text.trim()) return false;
    if (Array.isArray(value)) return value.length >= Math.max(1, field.minItems ?? 1) && value.length <= (field.maxItems ?? Infinity);
    return typeof value === 'string' && value.length >= Math.max(1, field.minLength ?? 1) && value.length <= (field.maxLength ?? Infinity);
  };
  const answered = fields.filter(valid).length;
  const summary = request.state?.status === 'answered' ? request.state.answer : submitted;
  const submit = async () => {
    if (sendingRef.current || answered !== fields.length || fields.length === 0) return;
    sendingRef.current = true;
    setSending(true);
    setError(null);
    const values = Object.fromEntries(fields.map((field) => [field.key, answer[field.key]]));
    try {
      await onSubmit(values);
      setSubmitted(values);
    } catch (failure) {
      setError(getErrorMessage(failure));
    } finally {
      sendingRef.current = false;
      setSending(false);
    }
  };
  const titleFor = (field: QuestionField) => field.title || request.title;
  if (summary) {
    return (
      <section className="app-question app-question--answered" aria-label={S.questions.answered}>
        <h3 className="app-question__heading">{S.questions.answered}</h3>
        <dl className="app-question__summary">
          {request.fields.filter((field) => Object.prototype.hasOwnProperty.call(summary, field.key)).map((field) => {
            const value = summary[field.key];
            const labels = (Array.isArray(value) ? value : [String(value)]).map((entry) => field.options?.find((option) => option.value === entry)?.label ?? entry);
            return <div key={field.key}><dt>{titleFor(field)}</dt><dd>{labels.join(', ')}</dd></div>;
          })}
        </dl>
      </section>
    );
  }
  if (request.state?.status === 'cancelled') return <p className="app-question">{S.questions.cancelled}</p>;
  return (
    <Form className="app-question" aria-label={S.questions.needed} onSubmit={(event) => { event.preventDefault(); void submit(); }}
      onKeyDown={(event) => {
        if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) { event.preventDefault(); void submit(); }
      }}>
      <div className="app-question__header">
        <h3 className="app-question__heading">{S.questions.needed}</h3>
        {fields.length > 1 ? <span aria-live="polite">{S.questions.progress.replace('{answered}', String(answered)).replace('{total}', String(fields.length))}</span> : null}
      </div>
      {fields.map((field, index) => {
        const draft = draftFor(field.key);
        const options = field.options ?? [];
        return (
          <fieldset key={field.key} className="app-question__section" disabled={sending}>
            <legend className="app-question__prompt">{fields.length > 1 ? `${index + 1} · ` : ''}{titleFor(field)}</legend>
            {field.description ? <p className="app-question__description">{field.description}</p> : null}
            {field.type === 'multiselect' ? <p className="app-question__hint">{S.questions.selectMultiple}</p> : null}
            <div className="app-question__options">
              {options.map((option, optionIndex) => {
                const recommended = /\s*\(Recommended\)$/i.test(option.label);
                const label = <span className="app-question__option-content"><span>{option.label.replace(/\s*\(Recommended\)$/i, '')}{recommended ? <span className="app-question__recommended">{S.questions.recommended}</span> : null}</span>{option.description ? <span className="app-question__description">{option.description}</span> : null}</span>;
                const checked = draft.choices.includes(option.value);
                const props = {
                  id: `${id}-${index}-${optionIndex}`,
                  label,
                  checked,
                  onChange: () => update(field.key, { choices: field.type === 'multiselect' ? checked ? draft.choices.filter((entry) => entry !== option.value) : [...draft.choices, option.value] : [option.value], custom: field.type === 'multiselect' ? draft.custom : false }),
                };
                return <div key={option.value} className={`app-question__option${checked ? ' is-selected' : ''}`}>{field.type === 'multiselect' ? <CheckboxInput {...props} /> : <RadioInput {...props} name={`${id}-${field.key}`} />}</div>;
              })}
              {options.length > 0 && field.custom !== false ? (
                <div className={`app-question__option${draft.custom ? ' is-selected' : ''}`}>
                  {field.type === 'multiselect' ? (
                    <CheckboxInput id={`${id}-${index}-custom`} label={S.questions.addAnother} checked={draft.custom} onChange={() => update(field.key, { custom: !draft.custom })} />
                  ) : (
                    <RadioInput id={`${id}-${index}-custom`} name={`${id}-${field.key}`} label={S.questions.writeAnother} checked={draft.custom} onChange={() => update(field.key, { custom: true, choices: [] })} />
                  )}
                </div>
              ) : null}
            </div>
            {options.length === 0 || draft.custom ? (
              <Textarea
                id={`${id}-${index}-text`}
                className="app-question__text"
                label={options.length ? S.questions.writeAnother : titleFor(field)}
                labelClassName="u-off-screen"
                placeholder={field.placeholder ?? S.questions.customPlaceholder}
                rows={3}
                onControlEnter={() => { void submit(); }}
                value={draft.text}
                onChange={(event) => update(field.key, { text: event.target.value })}
              />
            ) : null}
          </fieldset>
        );
      })}
      {error ? <p role="alert" className="app-question__error">{error}</p> : null}
      <div className="app-question__footer">
        {onDismiss ? <Button type="button" appearance="base" disabled={sending} onClick={async () => {
          if (sendingRef.current) return;
          sendingRef.current = true;
          setSending(true);
          setError(null);
          try { await onDismiss(); } catch (failure) { setError(getErrorMessage(failure)); }
          finally { sendingRef.current = false; setSending(false); }
        }}>{S.questions.dismiss}</Button> : null}
        <Button type="submit" appearance="positive" disabled={sending || answered !== fields.length || fields.length === 0}>
          {sending ? S.questions.sending : error ? S.questions.retry : fields.length > 1 ? S.questions.submitMany.replace('{count}', String(fields.length)) : S.questions.submitOne}
        </Button>
      </div>
    </Form>
  );
}
