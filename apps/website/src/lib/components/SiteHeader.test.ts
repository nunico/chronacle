import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import LanguageSwitch from './LanguageSwitch.svelte';
import SiteHeader from './SiteHeader.svelte';

const labels = {
  home: 'Chronacle home',
  manual: 'Manual',
  source: 'Source',
  download: 'Download',
  language: 'Language',
  english: 'English',
  german: 'German',
  navigation: 'Primary navigation',
};

describe('SiteHeader', () => {
  it('exposes the primary destinations and language control by name', () => {
    render(SiteHeader, {
      locale: 'en',
      labels,
      links: {
        home: '/',
        manual: '/en/manual',
        source: 'https://github.com/nunico/chronacle',
        download: '/#download',
      },
      onlocalechange: vi.fn(),
    });

    expect(screen.getByRole('link', { name: labels.home })).toHaveAttribute('href', '/');
    expect(screen.getByRole('link', { name: labels.manual })).toHaveAttribute('href', '/en/manual');
    expect(screen.getByRole('link', { name: labels.source })).toHaveAttribute(
      'href',
      'https://github.com/nunico/chronacle',
    );
    expect(screen.getByRole('group', { name: labels.language })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: labels.download })).toHaveAttribute(
      'href',
      '/#download',
    );
  });
});

describe('LanguageSwitch', () => {
  it('announces the selected locale and reports a German selection', async () => {
    const user = userEvent.setup();
    const onchange = vi.fn();

    render(LanguageSwitch, {
      value: 'en',
      label: labels.language,
      englishLabel: labels.english,
      germanLabel: labels.german,
      onchange,
    });

    expect(screen.getByRole('button', { name: labels.english })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    const germanButton = screen.getByRole('button', { name: labels.german });
    expect(germanButton).toHaveAttribute('aria-pressed', 'false');

    await user.click(germanButton);

    expect(onchange).toHaveBeenCalledOnce();
    expect(onchange).toHaveBeenCalledWith('de');
  });
});
