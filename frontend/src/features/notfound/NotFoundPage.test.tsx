import { render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import { axeOptions } from '../../testing/axe';
import { NotFoundPage } from './NotFoundPage';
import { S } from '../../strings/catalogue';

describe('NotFoundPage', () => {
  it('renders the not-found state with a link home', async () => {
    const { container } = render(
      <MemoryRouter>
        <NotFoundPage />
      </MemoryRouter>,
    );
    expect(screen.getByText(S.notFound.title)).toBeInTheDocument();
    expect(screen.getByText(S.notFound.body)).toBeInTheDocument();
    expect(
      screen.getByRole('link', { name: S.notFound.goToProjects }),
    ).toBeInTheDocument();
    expect(await axe(container, axeOptions)).toHaveNoViolations();
  });
});