import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { i18n } from '../lib/locale.svelte';
import SaveStatus from './SaveStatus.svelte';

afterEach(() => {
  i18n.setLocale('en');
});

describe('SaveStatus', () => {
  it.each([
    ['pending', 'Unsaved changes'],
    ['saving', 'Saving…'],
    ['saved', 'Saved'],
  ] as const)('announces %s state politely as %s', (status, label) => {
    render(SaveStatus, { props: { status } });

    const announcement = screen.getByRole('status');
    expect(announcement).toHaveAttribute('aria-live', 'polite');
    expect(announcement).toHaveTextContent(label);
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('does not present an unacknowledged retained draft as saved domain content', () => {
    render(SaveStatus, { props: { status: 'pending', retainedThisSession: true } });

    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
    expect(screen.getByText('Retained for this session only.')).toBeInTheDocument();
    expect(screen.queryByText('Saved')).not.toBeInTheDocument();
  });

  it('keeps an actionable failure and its detail in an alert', () => {
    render(SaveStatus, {
      props: {
        status: 'failed',
        error: 'The entity is no longer available.',
        onRetry: vi.fn(),
        onDiscard: vi.fn(),
      },
    });

    const alert = screen.getByRole('alert');
    expect(alert).toHaveTextContent("Couldn't save");
    expect(alert).toHaveTextContent('The entity is no longer available.');
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Discard changes' })).toBeInTheDocument();
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('supports keyboard activation for Retry and Discard actions', async () => {
    const user = userEvent.setup();
    const onRetry = vi.fn();
    const onDiscard = vi.fn();
    render(SaveStatus, {
      props: {
        status: 'failed',
        error: 'database locked',
        onRetry,
        onDiscard,
      },
    });

    const retry = screen.getByRole('button', { name: 'Retry' });
    retry.focus();
    await user.keyboard('{Enter}');
    expect(onRetry).toHaveBeenCalledOnce();

    const discard = screen.getByRole('button', { name: 'Discard changes' });
    discard.focus();
    await user.keyboard(' ');
    expect(onDiscard).toHaveBeenCalledOnce();
  });

  it('keeps Discard visible but unavailable during an active save, then re-enables it', async () => {
    const user = userEvent.setup();
    const onDiscard = vi.fn();
    const blockedReason = 'Wait for saving to finish before discarding changes';
    const rendered = render(SaveStatus, {
      props: {
        status: 'saving',
        onDiscard,
        discardBlocked: true,
      },
    });

    const discard = screen.getByRole('button', { name: 'Discard changes' });
    expect(discard).toBeVisible();
    expect(discard).toBeDisabled();
    expect(discard).toHaveAccessibleDescription(blockedReason);

    const announcement = screen.getByRole('status');
    expect(announcement).toHaveAttribute('aria-live', 'polite');
    expect(announcement).toHaveTextContent(blockedReason);

    await user.click(discard);
    expect(onDiscard).not.toHaveBeenCalled();

    await rendered.rerender({
      status: 'pending',
      onDiscard,
      discardBlocked: false,
    });

    const settledDiscard = screen.getByRole('button', { name: 'Discard changes' });
    expect(settledDiscard).toBe(discard);
    expect(settledDiscard).toBeEnabled();
    expect(settledDiscard).not.toHaveAccessibleDescription(blockedReason);
    expect(screen.getByRole('status')).not.toHaveTextContent(blockedReason);

    settledDiscard.focus();
    await user.keyboard(' ');
    expect(onDiscard).toHaveBeenCalledOnce();
  });

  it('localizes the accessible active-save discard explanation', () => {
    i18n.setLocale('de');
    const blockedReason = i18n.t('drafts.waitForSavingBeforeDiscard');
    render(SaveStatus, {
      props: {
        status: 'saving',
        onDiscard: vi.fn(),
        discardBlocked: true,
      },
    });

    expect(blockedReason).not.toBe('Wait for saving to finish before discarding changes');
    expect(blockedReason).not.toBe('drafts.waitForSavingBeforeDiscard');
    expect(screen.getByRole('button', { name: 'Änderungen verwerfen' })).toBeDisabled();
    expect(screen.getByRole('status')).toHaveTextContent(blockedReason);
  });

  it('renders status and recovery controls from the active locale', () => {
    i18n.setLocale('de');
    render(SaveStatus, {
      props: {
        status: 'failed',
        error: 'Datenbank gesperrt',
        onRetry: vi.fn(),
        onDiscard: vi.fn(),
      },
    });

    expect(screen.getByRole('alert')).toHaveTextContent('Speichern nicht möglich');
    expect(screen.getByRole('button', { name: 'Erneut versuchen' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Änderungen verwerfen' })).toBeInTheDocument();
  });
});
