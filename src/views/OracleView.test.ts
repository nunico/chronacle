import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import OracleView from './OracleView.svelte';
import * as commands from '../lib/commands';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('../lib/commands', () => ({
  getChatHistory: vi.fn().mockResolvedValue([]),
  chatSend: vi.fn().mockResolvedValue(undefined),
  getChunkForCitation: vi.fn().mockResolvedValue(null),
}));

const m = vi.mocked(commands);

describe('OracleView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    m.getChatHistory.mockResolvedValue([]);
  });

  it('shows suggestion chips when the thread is empty', async () => {
    render(OracleView, {
      props: { activeCampaignId: null, onOpenUpload: vi.fn() },
    });
    expect(
      await screen.findByRole('button', { name: /Can I cast a spell while grappled/i }),
    ).toBeTruthy();
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
    });
  });

  it('Enter submits, calling chatSend with the active campaign id', async () => {
    render(OracleView, {
      props: { activeCampaignId: 'camp-1', onOpenUpload: vi.fn() },
    });
    const input = await screen.findByPlaceholderText('Ask a rule, a name, a place…');
    await fireEvent.input(input, { target: { value: 'How does cover work?' } });
    await fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => {
      expect(m.chatSend).toHaveBeenCalledWith('How does cover work?', 'camp-1');
    });
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
});
