import { describe, expect, it } from 'vitest';
import { formatDuration } from './duration';

describe('formatDuration', () => {
  it('keeps a decimal below a second so a quick thought is not shown as zero', () => {
    expect(formatDuration(189)).toBe('0.2s');
    expect(formatDuration(999)).toBe('1.0s');
  });

  it('rounds to whole seconds above one', () => {
    expect(formatDuration(1000)).toBe('1s');
    expect(formatDuration(1210)).toBe('1s');
    expect(formatDuration(5000)).toBe('5s');
    expect(formatDuration(7980)).toBe('8s');
  });

  it('switches to minutes and seconds past a minute', () => {
    expect(formatDuration(60_000)).toBe('1m');
    expect(formatDuration(65_000)).toBe('1m 5s');
    expect(formatDuration(3_723_000)).toBe('62m 3s');
  });

  it('says nothing for a duration it cannot trust', () => {
    expect(formatDuration(-1)).toBe('');
    expect(formatDuration(Number.NaN)).toBe('');
    expect(formatDuration(Number.POSITIVE_INFINITY)).toBe('');
  });
});
