import type { RunOptions } from 'axe-core';

const REACT_COMPONENTS_RULES: NonNullable<RunOptions['rules']> = {
  'aria-allowed-attr': { enabled: false },
  'aria-required-children': { enabled: false },
  'aria-required-parent': { enabled: false },
  'aria-valid-attr-value': { enabled: false },
  'landmark-no-duplicate-banner': { enabled: false },
  'landmark-unique': { enabled: false },
};

export const axeOptions: RunOptions = { rules: REACT_COMPONENTS_RULES };
