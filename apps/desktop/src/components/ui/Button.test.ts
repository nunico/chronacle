import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import Button from './Button.svelte';

describe('Button', () => {
  it('renders a disabled loading button without invoking its click handler', async () => {
    const onclick = vi.fn();
    const user = userEvent.setup();

    render(Button, { props: { loading: true, onclick, ariaLabel: 'Save' } });

    const button = screen.getByRole('button', { name: 'Saving…' });
    expect(button).toBeDisabled();

    await user.click(button);
    expect(onclick).not.toHaveBeenCalled();
  });

  it('requires an accessible label for icon-only buttons', () => {
    expect(() => render(Button, { props: { iconOnly: true } })).toThrow(
      'Icon-only buttons require an ariaLabel',
    );
  });

  it('normalizes an icon-only accessible label before rendering it', () => {
    render(Button, { props: { iconOnly: true, ariaLabel: '  Close dialog  ' } });

    expect(screen.getByRole('button', { name: 'Close dialog' })).toHaveAttribute(
      'aria-label',
      'Close dialog',
    );
  });

  it('keeps long localized loading labels in a single-line label element', () => {
    render(Button, {
      props: {
        loading: true,
        loadingText: 'Wird gespeichert und mit dem Server synchronisiert…',
        ariaLabel: 'Speichern',
      },
    });

    const button = screen.getByRole('button', { name: /Wird gespeichert/ });
    expect(button).toHaveClass('single-line');
    expect(button.querySelector('.button-label')).toHaveTextContent(
      'Wird gespeichert und mit dem Server synchronisiert…',
    );
  });
});
