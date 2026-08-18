import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Page from './+page.svelte';

const downloadUrl = 'https://github.com/nunico/chronacle/releases/latest';
const licenseUrl = 'https://github.com/nunico/chronacle/blob/main/LICENSE';

function prefer(language: string): void {
  vi.spyOn(window.navigator, 'languages', 'get').mockReturnValue([language]);
}

afterEach(() => {
  vi.restoreAllMocks();
  document.documentElement.lang = '';
  document.title = '';
});

describe('Chronacle landing page', () => {
  it('switches every visible landing section to German in place', async () => {
    prefer('en-US');
    const user = userEvent.setup();

    render(Page);
    await user.click(screen.getByRole('button', { name: 'Deutsch' }));

    expect(screen.getByText('Frag deine Bücher. Prüf die Antwort.')).toBeInTheDocument();
    expect(screen.getByText('Das Aufstehen kostet 4,5 Meter Bewegung.')).toBeInTheDocument();
    expect(screen.getByText('Antworten mit Fundstelle')).toBeInTheDocument();
    expect(screen.getByText('Frag, wie du am Tisch fragst')).toBeInTheDocument();
    expect(
      screen.getByRole('heading', { name: 'Deine Bibliothek bleibt auf diesem Rechner.' }),
    ).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Chronacle herunterladen' })).toBeInTheDocument();
    expect(screen.getAllByRole('link', { name: 'Handbuch' })).toHaveLength(2);
    expect(screen.getAllByRole('link', { name: 'Quellcode' })).toHaveLength(2);
    expect(screen.getByText('Quellen prüfen statt Antworten erraten.')).toBeInTheDocument();
    expect(document.title).toBe('Chronacle — belegte Antworten aus deinen Büchern');
    expect(document.querySelector('meta[name="description"]')).toHaveAttribute(
      'content',
      'Lade Quellen als PDF, führe Kampagnennotizen und stell Fragen mit prüfbaren Fundstellen.',
    );
    const licenseLink = screen.getByRole('link', { name: 'Lizenz' });
    expect(licenseLink).toHaveAttribute('href', licenseUrl);
    expect(licenseLink).toHaveAttribute('target', '_blank');
    expect(licenseLink).toHaveAttribute('rel', 'external noopener noreferrer');
    expect(screen.getByRole('link', { name: 'Open Game License' })).toHaveAttribute(
      'href',
      '/legal/open-game-license',
    );
  });

  it('updates the manual destination when the locale changes', async () => {
    prefer('en-US');
    const user = userEvent.setup();

    render(Page);

    const englishManualLinks = document.querySelectorAll('a[href="/en/manual"]');
    expect(englishManualLinks).toHaveLength(4);

    await user.click(screen.getByRole('button', { name: 'Deutsch' }));

    const germanManualLinks = document.querySelectorAll('a[href="/de/handbuch"]');
    expect(germanManualLinks).toHaveLength(4);
  });

  it('updates the document language after a language selection', async () => {
    prefer('en-US');
    const user = userEvent.setup();

    render(Page);
    await user.click(screen.getByRole('button', { name: 'Deutsch' }));

    expect(document.documentElement).toHaveAttribute('lang', 'de');
  });

  it('uses browser preference without reading or writing persistent browser state', async () => {
    prefer('en-US');
    const user = userEvent.setup();
    const localStorageSpy = vi.spyOn(window, 'localStorage', 'get');
    const sessionStorageSpy = vi.spyOn(window, 'sessionStorage', 'get');
    const storageGetSpy = vi.spyOn(Storage.prototype, 'getItem');
    const storageSetSpy = vi.spyOn(Storage.prototype, 'setItem');
    const cookieGetSpy = vi.spyOn(Document.prototype, 'cookie', 'get');
    const cookieSetSpy = vi.spyOn(Document.prototype, 'cookie', 'set');

    render(Page);
    await user.click(screen.getByRole('button', { name: 'Deutsch' }));

    expect(await screen.findByText('Frag deine Bücher. Prüf die Antwort.')).toBeInTheDocument();
    expect(localStorageSpy).not.toHaveBeenCalled();
    expect(sessionStorageSpy).not.toHaveBeenCalled();
    expect(storageGetSpy).not.toHaveBeenCalled();
    expect(storageSetSpy).not.toHaveBeenCalled();
    expect(cookieGetSpy).not.toHaveBeenCalled();
    expect(cookieSetSpy).not.toHaveBeenCalled();
  });

  it('uses the approved accessible vector mark in the hero', () => {
    prefer('en-US');

    render(Page);

    expect(screen.getByRole('img', { name: 'Chronacle' }).tagName).toBe('svg');
  });

  it('uses the latest release URL for security-safe external download links', () => {
    prefer('en-US');

    render(Page);

    const downloadLinks = screen.getAllByRole('link', { name: 'Download Chronacle' });
    expect(downloadLinks).toHaveLength(3);
    for (const link of downloadLinks) {
      expect(link).toHaveAttribute('href', downloadUrl);
      expect(link).toHaveAttribute('target', '_blank');
      expect(link).toHaveAttribute('rel', 'external noopener noreferrer');
    }
  });
});
