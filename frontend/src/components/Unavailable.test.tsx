import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import { axeOptions } from '../testing/axe';
import { Unavailable } from './Unavailable';
import { S } from '../strings/catalogue';

describe('Unavailable', () => {
  it('renders the not-available notice', async () => {
    const { container } = render(<Unavailable />);
    expect(screen.getByText(S.unavailable.title)).toBeInTheDocument();
    expect(screen.getByText(S.unavailable.body)).toBeInTheDocument();
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });
});