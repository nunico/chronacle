import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const route = vi.hoisted(() => ({ pathname: '/missing' }));

vi.mock('$app/state', () => ({
  page: {
    error: { message: 'Not found' },
    status: 404,
    url: {
      get pathname() {
        return route.pathname;
      },
    },
  },
}));

import ErrorPage from './+error.svelte';

describe('website error page', () => {
  beforeEach(() => {
    route.pathname = '/missing';
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('falls back to English and links home and to the manual overview', () => {
    render(ErrorPage);

    expect(document.querySelector('base')).toHaveAttribute('href', '/');
    expect(screen.getByRole('heading', { name: 'That page is not here.' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Chronacle home' })).toHaveAttribute('href', '/');
    expect(screen.getByRole('link', { name: 'Manual overview' })).toHaveAttribute(
      'href',
      '/en/manual',
    );
    expect(screen.queryByRole('button', { name: 'Search the manual' })).not.toBeInTheDocument();
  });

  it('derives German copy from a German route without persisting it', () => {
    route.pathname = '/de/irgendwo';
    const storageGetSpy = vi.spyOn(Storage.prototype, 'getItem');
    const storageSetSpy = vi.spyOn(Storage.prototype, 'setItem');
    const cookieGetSpy = vi.spyOn(Document.prototype, 'cookie', 'get');
    const cookieSetSpy = vi.spyOn(Document.prototype, 'cookie', 'set');

    render(ErrorPage);

    expect(screen.getByRole('heading', { name: 'Diese Seite gibt es nicht.' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Zur Chronacle-Startseite' })).toHaveAttribute(
      'href',
      '/',
    );
    expect(screen.getByRole('link', { name: 'Handbuchüberblick' })).toHaveAttribute(
      'href',
      '/de/handbuch',
    );
    expect(storageGetSpy).not.toHaveBeenCalled();
    expect(storageSetSpy).not.toHaveBeenCalled();
    expect(cookieGetSpy).not.toHaveBeenCalled();
    expect(cookieSetSpy).not.toHaveBeenCalled();
  });

  it('offers the current-language search trigger on a manual 404', async () => {
    route.pathname = '/de/handbuch/fehlt';
    const user = userEvent.setup();

    render(ErrorPage);
    const trigger = screen.getByRole('button', { name: 'Handbuch durchsuchen' });
    await user.click(trigger);

    expect(screen.getByRole('dialog', { name: 'Handbuch durchsuchen' })).toBeInTheDocument();
    expect(screen.getByRole('combobox', { name: 'Handbuch durchsuchen' })).toHaveFocus();
  });
});
