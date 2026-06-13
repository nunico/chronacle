import { render } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import ExtractionCard from './ExtractionCard.svelte';

describe('ExtractionCard', () => {
  it('shows the current phase detail while running', () => {
    const { getByText, getByRole } = render(ExtractionCard, {
      props: {
        status: 'running',
        title: 'Extracting "Commander Varn"',
        detail: 'Found 12 passages',
        entitiesFound: 0,
        relationsFound: 0,
        onCancel: () => {},
      },
    });
    expect(getByText('Found 12 passages')).toBeTruthy();
    expect(getByRole('button', { name: /cancel/i })).toBeTruthy();
  });

  it('shows a result summary on success and hides cancel', () => {
    const { getByText, queryByRole } = render(ExtractionCard, {
      props: {
        status: 'done',
        title: 'Extraction complete',
        detail: 'Created 5 entities, 4 relations',
        entitiesFound: 5,
        relationsFound: 4,
        onCancel: () => {},
      },
    });
    expect(getByText('Created 5 entities, 4 relations')).toBeTruthy();
    expect(queryByRole('button', { name: /cancel/i })).toBeNull();
  });

  it('renders the cancelled terminal state with kept counts', () => {
    const { getByText } = render(ExtractionCard, {
      props: {
        status: 'cancelled',
        title: 'Cancelled',
        detail: 'Cancelled — kept 2 entities / 1 relations created so far',
        entitiesFound: 2,
        relationsFound: 1,
        onCancel: () => {},
      },
    });
    expect(getByText(/kept 2 entities/)).toBeTruthy();
  });

  it('renders the empty terminal state', () => {
    const { getByText } = render(ExtractionCard, {
      props: {
        status: 'empty',
        title: 'Nothing found',
        detail: 'No passages found for "Ghost"',
        entitiesFound: 0,
        relationsFound: 0,
        onCancel: () => {},
      },
    });
    expect(getByText('No passages found for "Ghost"')).toBeTruthy();
  });
});
