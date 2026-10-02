import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { QuestionCard } from './QuestionCard';
import type { QuestionForm } from '../../../lib/api/forms';
import { S } from '../../../strings/catalogue';

const request: QuestionForm = { id: 'frm_1', sessionID: 'ses_1', title: 'Choose a database', fields: [{ key: 'db', type: 'string', options: [{ value: 'pg', label: 'PostgreSQL (Recommended)', description: 'For production' }, { value: 'sqlite', label: 'SQLite' }], custom: true }] };

beforeEach(() => sessionStorage.clear());

describe('QuestionCard', () => {
  it('requires an explicit choice and submits once with option values', async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<QuestionCard request={request} onSubmit={onSubmit} />);
    expect(screen.getByRole('button', { name: S.questions.submitOne })).toHaveAttribute('aria-disabled', 'true');
    await user.click(screen.getByRole('radio', { name: /PostgreSQL/ }));
    expect(onSubmit).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: S.questions.submitOne }));
    expect(onSubmit).toHaveBeenCalledWith({ db: 'pg' });
    expect(await screen.findByRole('region', { name: S.questions.answered })).toHaveTextContent('PostgreSQL');
  });

  it('allows a custom single answer', async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<QuestionCard request={request} onSubmit={onSubmit} />);
    await user.click(screen.getByRole('radio', { name: S.questions.writeAnother }));
    await user.type(screen.getByRole('textbox'), '  MySQL  ');
    await user.click(screen.getByRole('button', { name: S.questions.submitOne }));
    expect(onSubmit).toHaveBeenCalledWith({ db: 'MySQL' });
  });

  it('combines multiple choices and a custom answer', async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<QuestionCard request={{ ...request, fields: [{ ...request.fields[0], type: 'multiselect' }] }} onSubmit={onSubmit} />);
    await user.click(screen.getByRole('checkbox', { name: /PostgreSQL/ }));
    await user.click(screen.getByRole('checkbox', { name: 'SQLite' }));
    await user.click(screen.getByRole('checkbox', { name: S.questions.addAnother }));
    await user.type(screen.getByRole('textbox'), 'MySQL');
    await user.click(screen.getByRole('button', { name: S.questions.submitOne }));
    expect(onSubmit).toHaveBeenCalledWith({ db: ['pg', 'sqlite', 'MySQL'] });
  });

  it('requires all questions, preserves draft on error and retries', async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockRejectedValueOnce(new Error('Connection failed')).mockResolvedValue(undefined);
    render(<QuestionCard request={{ ...request, fields: [...request.fields, { key: 'behavior', title: 'Failure handling', type: 'string' }] }} onSubmit={onSubmit} />);
    await user.click(screen.getByRole('radio', { name: 'SQLite' }));
    expect(screen.getByText('1 of 2 answered')).toBeInTheDocument();
    await user.type(screen.getByRole('textbox', { name: 'Failure handling' }), 'Offer a retry');
    await user.click(screen.getByRole('button', { name: 'Submit 2 answers' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('Connection failed');
    expect(screen.getByRole('textbox')).toHaveValue('Offer a retry');
    await user.click(screen.getByRole('button', { name: S.questions.retry }));
    expect(onSubmit).toHaveBeenLastCalledWith({ db: 'sqlite', behavior: 'Offer a retry' });
    await waitFor(() => expect(screen.getByRole('region', { name: S.questions.answered })).toBeInTheDocument());
  });

  it('prevents duplicate submissions while the request is pending', async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn(() => new Promise<void>(() => {}));
    render(<QuestionCard request={request} onSubmit={onSubmit} />);
    await user.click(screen.getByRole('radio', { name: 'SQLite' }));
    await user.dblClick(screen.getByRole('button', { name: S.questions.submitOne }));
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: S.questions.sending })).toHaveAttribute('aria-disabled', 'true');
  });

  it('supports dismissal without submitting answers', async () => {
    const user = userEvent.setup();
    const onDismiss = vi.fn().mockResolvedValue(undefined);
    const onSubmit = vi.fn();
    render(<QuestionCard request={request} onSubmit={onSubmit} onDismiss={onDismiss} />);
    await user.click(screen.getByRole('button', { name: S.questions.dismiss }));
    expect(onDismiss).toHaveBeenCalledTimes(1);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('submits free-form text with ctrl-enter and retains newlines', async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    render(<QuestionCard request={{ ...request, fields: [{ key: 'text', type: 'string' }] }} onSubmit={onSubmit} />);
    await user.type(screen.getByRole('textbox'), 'First{Enter}Second');
    await user.keyboard('{Control>}{Enter}{/Control}');
    expect(onSubmit).toHaveBeenCalledWith({ text: 'First\nSecond' });
  });
});
