import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import OracleView from './OracleView.svelte';
import * as commands from '../lib/commands';
import { toasts, clearToasts } from '../lib/toast.svelte';
import { i18n } from '../lib/locale.svelte';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('../lib/commands', () => ({
  getChatHistory: vi.fn().mockResolvedValue([]),
  chatSend: vi.fn().mockResolvedValue(undefined),
  chatCancel: vi.fn().mockResolvedValue(undefined),
  getChunkForCitation: vi.fn().mockResolvedValue(null),
  getSources: vi.fn().mockResolvedValue([]),
  extractEntityByName: vi.fn().mockResolvedValue({ entities_created: 0, relations_created: 0 }),
  extractAllFromCampaign: vi.fn().mockResolvedValue({ entities_created: 0, relations_created: 0 }),
  cancelExtraction: vi.fn().mockResolvedValue(undefined),
  saveChatToCodex: vi.fn().mockResolvedValue(0),
}));

const m = vi.mocked(commands);

const SAMPLE_SOURCE = {
  id: 's1',
  filename: 'srd.pdf',
  display_name: 'SRD 5.2',
  source_type: 'rules',
  index_status: 'indexed',
} as unknown as commands.Source;

describe('OracleView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    clearToasts();
    m.getChatHistory.mockResolvedValue([]);
    m.chatSend.mockResolvedValue(undefined);
    m.getSources.mockResolvedValue([SAMPLE_SOURCE]);
    m.saveChatToCodex.mockResolvedValue(0);
  });

  it('shows suggestion chips when the thread is empty', async () => {
    render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload: vi.fn() },
    });
    expect(
      await screen.findByRole('button', { name: /Can I cast a spell while grappled/i }),
    ).toBeTruthy();
  });

  it('uses the active locale for composer and source controls', async () => {
    i18n.setLocale('de');
    try {
      render(OracleView, {
        props: { activeCampaignId: null, onOpenUpload: vi.fn() },
      });

      expect(
        await screen.findByPlaceholderText('Frage nach einer Regel, einem Namen, einem Ort…'),
      ).toBeTruthy();
      expect(screen.getByRole('button', { name: 'Regelbuch anhängen' })).toBeTruthy();
    } finally {
      i18n.setLocale('en');
    }
  });

  it('hides suggestions once a message exists', async () => {
    m.getChatHistory.mockResolvedValue([{ role: 'user', content: 'hi' }]);
    render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload: vi.fn() },
    });
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: /spell while grappled/i })).toBeNull();
    });
  });

  it('renders a ruling card for an assistant message with a [Source] citation', async () => {
    m.getChatHistory.mockResolvedValue([
      {
        role: 'assistant',
        content:
          'Yes, but at disadvantage. The grapple imposes disadvantage on the roll. [Source: "SRD 5.2", p.190, quote: "Speed becomes 0."]',
      },
    ]);
    render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload: vi.fn() },
    });
    await waitFor(() => {
      expect(screen.getByText(/Yes, but at disadvantage/i)).toBeTruthy();
      expect(screen.getAllByRole('button', { name: /SRD 5\.2 p\.190/i }).length).toBeGreaterThan(0);
      expect(screen.getByRole('button', { name: /Save to Codex/i })).toBeTruthy();
    });
  });

  it("does not leak a card's expand state to a different message at the same index after reload", async () => {
    // Ruling A at index 0, with a collapsible citation.
    m.getChatHistory.mockResolvedValue([
      { role: 'assistant', content: 'Answer A. [Source: "BookA", p.1, quote: "Alpha passage."]' },
    ]);
    const { container, rerender } = render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
    });

    // Expand the citation on ruling A via the RulingCard toggle (button.cite).
    const citeToggle = await waitFor(() => {
      const btn = container.querySelector('button.cite');
      if (!btn) throw new Error('cite toggle not rendered yet');
      return btn;
    });
    await fireEvent.click(citeToggle);
    // The expanded passage shows as visible text (getByText ignores data-quote attrs).
    await waitFor(() => expect(screen.getByText('Alpha passage.')).toBeTruthy());

    // Reload the thread (campaign switch) with a *different* ruling at index 0.
    m.getChatHistory.mockResolvedValue([
      { role: 'assistant', content: 'Answer B. [Source: "BookB", p.2, quote: "Beta passage."]' },
    ]);
    await rerender({ activeCampaignId: 'camp-2', onOpenUpload: vi.fn() });

    // The new card must render collapsed — keying by message object (not index)
    // gives it a fresh instance instead of inheriting A's expanded state.
    await waitFor(() => expect(screen.getByText(/Answer B/)).toBeTruthy());
    expect(screen.queryByText('Beta passage.')).toBeNull();
    expect(screen.queryByText('Alpha passage.')).toBeNull();
  });

  it('resolves the response language from the sent message before calling chatSend', async () => {
    i18n.setLocale('de');
    try {
      render(OracleView, {
        props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
      });
      const input = await screen.findByPlaceholderText('Frage nach einer Regel, einem Namen, einem Ort…');
      await fireEvent.input(input, { target: { value: 'Quelle est la règle ?' } });
      await fireEvent.keyDown(input, { key: 'Enter' });
      await waitFor(() => {
        expect(m.chatSend).toHaveBeenCalledWith('Quelle est la règle ?', 'camp-1', 'fr');
      });
    } finally {
      i18n.setLocale('en');
    }
  });

  it('the paperclip button triggers onOpenUpload', async () => {
    const onOpenUpload = vi.fn();
    render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload },
    });
    const paperclip = await screen.findByRole('button', { name: /Attach a rulebook/i });
    await fireEvent.click(paperclip);
    expect(onOpenUpload).toHaveBeenCalled();
  });

  it('uses a multiline textarea; Shift+Enter does not send', async () => {
    render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
    });
    const input = await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    expect(input.tagName).toBe('TEXTAREA');
    await fireEvent.input(input, { target: { value: 'line one' } });
    await fireEvent.keyDown(input, { key: 'Enter', shiftKey: true });
    expect(m.chatSend).not.toHaveBeenCalled();
  });

  it('keeps the composer enabled and shows a stop button while a response streams', async () => {
    // chatSend resolves but no done token arrives — the view stays loading.
    render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
    });
    const input = await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    await fireEvent.input(input, { target: { value: 'How does cover work?' } });
    await fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /stop generating/i })).toBeTruthy();
    });
    expect((input as HTMLTextAreaElement).disabled).toBe(false);
    await fireEvent.click(screen.getByRole('button', { name: /stop generating/i }));
    expect(m.chatCancel).toHaveBeenCalled();
  });

  it('shows an error bubble with a retry button when chatSend rejects', async () => {
    m.chatSend.mockRejectedValueOnce(new Error('connection refused'));
    render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
    });
    const input = await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    await fireEvent.input(input, { target: { value: 'How does cover work?' } });
    await fireEvent.keyDown(input, { key: 'Enter' });
    const retry = await screen.findByRole('button', { name: /retry/i });
    expect(screen.getByText(/connection refused/)).toBeTruthy();
    await fireEvent.click(retry);
    await waitFor(() => {
      expect(m.chatSend).toHaveBeenCalledTimes(2);
      expect(m.chatSend).toHaveBeenLastCalledWith('How does cover work?', 'camp-1', 'en');
    });
  });

  it('renders backend [Error: ...] messages as an error bubble with retry', async () => {
    m.getChatHistory.mockResolvedValue([
      { role: 'user', content: 'How does cover work?' },
      { role: 'assistant', content: '[Error: LLM unreachable]' },
    ]);
    render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
    });
    expect(await screen.findByText(/LLM unreachable/)).toBeTruthy();
    expect(screen.getByRole('button', { name: /retry/i })).toBeTruthy();
  });

  it('nudges the user to upload a rulebook when no sources are indexed', async () => {
    m.getSources.mockResolvedValue([]);
    const onOpenUpload = vi.fn();
    render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload },
    });
    const nudge = await screen.findByRole('button', { name: /upload a rulebook/i });
    expect(screen.queryByRole('button', { name: /spell while grappled/i })).toBeNull();
    await fireEvent.click(nudge);
    expect(onOpenUpload).toHaveBeenCalled();
  });

  it('does not advertise dice rolling in the suggestion pills', async () => {
    render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload: vi.fn() },
    });
    await screen.findByRole('button', { name: /spell while grappled/i });
    expect(screen.queryByRole('button', { name: /roll initiative/i })).toBeNull();
  });

  it('does not inject raw <script> when a citation source name is malicious', async () => {
    m.getChatHistory.mockResolvedValue([
      {
        role: 'assistant',
        content: 'Foo. [Source: "<script>alert(1)</script>", p.1]',
      },
    ]);
    const { container } = render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload: vi.fn() },
    });
    await waitFor(() => {
      expect(container.querySelector('script')).toBeNull();
    });
  });

  it('routes /extract <name> to extractEntityByName, not chatSend', async () => {
    render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
    });
    const input = await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    await fireEvent.input(input, { target: { value: '/extract Commander Varn' } });
    await fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => {
      expect(m.extractEntityByName).toHaveBeenCalledWith('camp-1', 'Commander Varn');
    });
    expect(m.chatSend).not.toHaveBeenCalled();
  });

  it('bare /extract shows a usage hint and starts no extraction', async () => {
    render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
    });
    const input = await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    await fireEvent.input(input, { target: { value: '/extract' } });
    await fireEvent.keyDown(input, { key: 'Enter' });
    await screen.findByText(/Usage: \/extract/i);
    expect(m.extractEntityByName).not.toHaveBeenCalled();
    expect(m.extractAllFromCampaign).not.toHaveBeenCalled();
    expect(m.chatSend).not.toHaveBeenCalled();
  });

  it('assistant message shows Save to Codex action', async () => {
    m.getChatHistory.mockResolvedValue([{ role: 'assistant', content: 'The oracle speaks.' }]);
    render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
    });
    expect(await screen.findByRole('button', { name: /Save to Codex/i })).toBeTruthy();
  });

  it('clicking Save to Codex invokes save_chat_to_codex with campaign id and content, then shows a toast', async () => {
    m.getChatHistory.mockResolvedValue([{ role: 'assistant', content: 'The oracle speaks.' }]);
    m.saveChatToCodex.mockResolvedValue(2);
    const onSavedToCodex = vi.fn();
    render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn(), onSavedToCodex },
    });
    const btn = await screen.findByRole('button', { name: /Save to Codex/i });
    await fireEvent.click(btn);
    await waitFor(() => {
      expect(m.saveChatToCodex).toHaveBeenCalledWith('camp-1', 'The oracle speaks.');
    });
    await waitFor(() => {
      expect(toasts.some((t) => t.message.includes('2 proposal'))).toBe(true);
    });
    expect(onSavedToCodex).toHaveBeenCalledWith(2);
  });

  it('does not show Save to Codex on a user message or an error message', async () => {
    m.getChatHistory.mockResolvedValue([
      { role: 'user', content: 'How does cover work?' },
      { role: 'error', content: 'Something went wrong.' },
    ]);
    render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
    });
    await screen.findByText('How does cover work?');
    await screen.findByText('Something went wrong.');
    expect(screen.queryByRole('button', { name: /Save to Codex/i })).toBeNull();
  });
});
