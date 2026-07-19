import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import ProgressBar from './ProgressBar.svelte';

describe('ProgressBar', () => {
  it('exposes determinate progress to assistive technology', () => {
    render(ProgressBar, { props: { value: 62, label: 'Indexing' } });

    expect(screen.getByRole('progressbar', { name: 'Indexing' })).toHaveAttribute(
      'aria-valuenow',
      '62',
    );
  });
});
