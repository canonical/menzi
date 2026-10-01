import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, act } from '@testing-library/react';
import { WorkingLabel } from './WorkingLabel';
import {
  LEADING_PHRASES,
  TRAILING_PHRASES,
  WORKING_INTERVAL_MS,
  WORKING_PHRASES,
  shuffledTrailing,
  workingSequence,
} from '../../strings/working';

describe('WorkingLabel', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('shows the first language', () => {
    render(<WorkingLabel />);
    expect(screen.getByTestId('working-label').textContent).toContain(WORKING_PHRASES[0].text);
  });

  it('starts with English', () => {
    expect(LEADING_PHRASES[0]).toEqual({ language: 'English', text: 'Working' });
  });

  it('keeps French second', () => {
    expect(LEADING_PHRASES[1]).toEqual({ language: 'Français', text: 'En train de travailler' });
    expect(WORKING_PHRASES[1]).toEqual(LEADING_PHRASES[1]);
  });

  it('changes language every interval', () => {
    render(<WorkingLabel />);
    const label = screen.getByTestId('working-label');
    expect(label.textContent).toContain('Working');

    act(() => {
      vi.advanceTimersByTime(WORKING_INTERVAL_MS);
    });
    expect(label.textContent).toContain('En train de travailler');

    act(() => {
      vi.advanceTimersByTime(WORKING_INTERVAL_MS);
    });
    expect(label.textContent).not.toContain('En train de travailler');
    expect(label.textContent).not.toContain('Working');
  });

  it('keeps changing rather than settling', () => {
    render(<WorkingLabel />);
    const label = screen.getByTestId('working-label');
    const seen = new Set<string>();

    for (let step = 0; step < 8; step += 1) {
      act(() => {
        vi.advanceTimersByTime(WORKING_INTERVAL_MS);
      });
      seen.add(label.textContent ?? '');
    }

    expect(seen.size).toBeGreaterThan(4);
  });

  it('wraps back to the first language', () => {
    render(<WorkingLabel />);
    const label = screen.getByTestId('working-label');

    act(() => {
      vi.advanceTimersByTime(WORKING_INTERVAL_MS * WORKING_PHRASES.length);
    });
    expect(label.textContent).toContain('Working');
  });

  it('does not repeat a language', () => {
    const texts = WORKING_PHRASES.map((phrase) => phrase.text);
    expect(new Set(texts).size).toBe(texts.length);
  });

  it('names each language for assistive technology', () => {
    render(<WorkingLabel />);
    expect(screen.getByText('Working')).toHaveAttribute('lang', 'English');
  });

  it('carries a stable sentence for a screen reader', () => {
    render(<WorkingLabel />);
    expect(screen.getByText('The agent is working.')).toBeInTheDocument();
  });

  it('announces politely rather than interrupting', () => {
    render(<WorkingLabel />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('stops cycling once unmounted', () => {
    const view = render(<WorkingLabel />);
    view.unmount();

    expect(() =>
      act(() => {
        vi.advanceTimersByTime(WORKING_INTERVAL_MS * 4);
      }),
    ).not.toThrow();
  });

  it('covers every continent', () => {
    const languages = WORKING_PHRASES.map((phrase) => phrase.language);
    for (const expected of [
      'English',
      'Français',
      '日本語',
      '中文',
      '한국어',
      'العربية',
      'हिन्दी',
      'ไทย',
      'Bahasa Indonesia',
      'Kiswahili',
      'isiZulu',
      'Amharic',
      'ትግርኛ',
      'Af-Soomaali',
      'Yorùbá',
      'Wolof',
      'Haitian Creole',
      'Quechua',
      'Mapudungun',
      'Māori',
      'ʻŌlelo Hawaiʻi',
      'Tok Pisin',
      'Cherokee',
      'Русский',
      'Ελληνικά',
    ]) {
      expect(languages).toContain(expected);
    }
  });
});

describe('workingSequence', () => {
  it('puts English first and French second', () => {
    const sequence = workingSequence();
    expect(sequence[0].language).toBe('English');
    expect(sequence[1].language).toBe('Français');
  });

  it('holds those two in place however the rest is shuffled', () => {
    const sequence = workingSequence(() => 0);
    expect(sequence[0].language).toBe('English');
    expect(sequence[1].language).toBe('Français');
  });

  it('includes every phrase exactly once', () => {
    const sequence = workingSequence();
    expect(sequence).toHaveLength(WORKING_PHRASES.length);
    expect(new Set(sequence.map((p) => p.text)).size).toBe(WORKING_PHRASES.length);
  });

  it('randomises the order after the first two', () => {
    const orders = new Set(
      Array.from({ length: 12 }, () => workingSequence().map((p) => p.text).join('|')),
    );
    expect(orders.size).toBeGreaterThan(1);
  });

  it('gives every trailing phrase a chance of landing first after the leaders', () => {
    const firsts = new Set(Array.from({ length: 200 }, () => workingSequence()[2].text));
    expect(firsts.size).toBeGreaterThan(20);
  });

  it('leaves the leading pair out of the shuffled pool', () => {
    const tail = shuffledTrailing(() => 0.5).map((p) => p.text);
    expect(tail).not.toContain('Working');
    expect(tail).not.toContain('En train de travailler');
  });

  it('keeps every trailing phrase in the pool', () => {
    const tail = shuffledTrailing(() => 0.5);
    expect(tail).toHaveLength(TRAILING_PHRASES.length);
    expect(new Set(tail.map((p) => p.text))).toEqual(
      new Set(TRAILING_PHRASES.map((p) => p.text)),
    );
  });

  it('produces a different order for different randomness', () => {
    const forwards = workingSequence(() => 0).map((p) => p.text).join('|');
    const backwards = workingSequence(() => 0.999).map((p) => p.text).join('|');
    expect(forwards).not.toBe(backwards);
  });

  it('does not mutate the source list', () => {
    const before = WORKING_PHRASES.map((p) => p.text).join('|');
    workingSequence();
    shuffledTrailing();
    expect(WORKING_PHRASES.map((p) => p.text).join('|')).toBe(before);
  });
});