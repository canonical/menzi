import { describe, expect, it, vi, beforeEach } from 'vitest';
import { navigateTo } from './navigation';

const assign = vi.fn();

beforeEach(() => {
  assign.mockClear();
  vi.stubGlobal('location', { assign });
});

describe('navigateTo', () => {
  it('navigates to the given path', () => {
    navigateTo('/projects');
    expect(assign).toHaveBeenCalledWith('/projects');
  });
});
