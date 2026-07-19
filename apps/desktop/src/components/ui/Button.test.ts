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
});
