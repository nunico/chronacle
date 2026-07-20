import { render, screen, fireEvent } from '@testing-library/svelte';
import { describe, it, expect, vi } from 'vitest';
import AliasField from './AliasField.svelte';
import { i18n } from '../lib/locale.svelte';

describe('AliasField', () => {
  it('uses the active display language for alternate-name controls', () => {
    i18n.setLocale('de');
    render(AliasField, { props: { aliases: [] } });

    expect(screen.getByText('Alternative Namen')).toBeInTheDocument();
    expect(screen.getByPlaceholderText('Alternativen Namen hinzufügen')).toBeInTheDocument();

    i18n.setLocale('en');
  });

  it('never says the word "aliases" to the GM; says "alternate names" instead', () => {
    render(AliasField, { props: { aliases: ['The Quassars'] } });
    expect(screen.queryByText(/alias/i)).not.toBeInTheDocument();
    expect(screen.getByText(/alternate names/i)).toBeInTheDocument();
  });

  it('renders one chip per alternate name', () => {
    render(AliasField, { props: { aliases: ['The Quassars', 'Quassar Clan'] } });
    expect(screen.getByText('The Quassars')).toBeInTheDocument();
    expect(screen.getByText('Quassar Clan')).toBeInTheDocument();
  });

  it('adding a name reports the COMPLETE array, not just the new entry', async () => {
    const onchange = vi.fn();
    render(AliasField, { props: { aliases: ['The Quassars'], onchange } });

    await fireEvent.input(screen.getByPlaceholderText('Add an alternate name'), {
      target: { value: 'Quassar Clan' },
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Add' }));

    expect(onchange).toHaveBeenCalledWith(['The Quassars', 'Quassar Clan']);
  });

  it('removing a name reports the remaining COMPLETE array, never null', async () => {
    const onchange = vi.fn();
    render(AliasField, {
      props: { aliases: ['The Quassars', 'Quassar Clan'], onchange },
    });

    await fireEvent.click(screen.getByRole('button', { name: 'Remove The Quassars' }));

    expect(onchange).toHaveBeenCalledWith(['Quassar Clan']);
    expect(onchange).not.toHaveBeenCalledWith(null);
  });

  it('does not add a blank or duplicate (case-insensitive) name', async () => {
    const onchange = vi.fn();
    render(AliasField, { props: { aliases: ['The Quassars'], onchange } });

    await fireEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(onchange).not.toHaveBeenCalled();

    await fireEvent.input(screen.getByPlaceholderText('Add an alternate name'), {
      target: { value: 'the quassars' },
    });
    await fireEvent.click(screen.getByRole('button', { name: 'Add' }));
    expect(onchange).not.toHaveBeenCalled();
  });

  it('renders with an empty alternate-name list', () => {
    render(AliasField, { props: { aliases: [] } });
    expect(screen.getByText(/alternate names/i)).toBeInTheDocument();
  });
});
