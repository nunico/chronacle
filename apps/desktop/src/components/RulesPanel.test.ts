import { render, screen, fireEvent, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import RulesPanel from './RulesPanel.svelte';
import type { RuleEntry } from '../lib/commands';
import { DraftCoordinator } from '../lib/drafts/draft-coordinator.svelte';
import { ruleScope, statusOf } from '../lib/drafts/draft-state';
import { i18n } from '../lib/locale.svelte';
import { rememberedRuleRecovery } from '../lib/drafts/rule-note-presentation';

vi.mock('../lib/commands', () => ({
  getRuleEntries: vi.fn().mockResolvedValue([]),
  updateRuleNotes: vi.fn().mockResolvedValue(undefined),
  redoRuleEntry: vi.fn().mockResolvedValue(undefined),
}));

import * as commands from '../lib/commands';
const m = vi.mocked(commands);

function rule(id: string, name: string, category: string): RuleEntry {
  return {
    id,
    name,
    category,
    body: `Body text for ${name}`,
    notes: null,
    page_refs: [{ source_name: 'PHB', page_start: 10, page_end: 12 }],
    stale: false,
  };
}

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function renderPanel(
  coordinator = new DraftCoordinator(),
  overrides: Partial<{ campaignId: string | null; collectionId: string }> = {},
) {
  return render(RulesPanel, {
    props: {
      campaignId: 'camp-a',
      collectionId: 'c-1',
      draftCoordinator: coordinator,
      ...overrides,
    },
  } as never);
}

async function openNotes(name = 'Initiative') {
  await fireEvent.click(await screen.findByRole('button', { name }));
  return screen.getByRole('textbox', { name: 'Table notes' });
}

describe('RulesPanel', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    i18n.setLocale('en');
    m.getRuleEntries.mockResolvedValue([]);
    m.updateRuleNotes.mockResolvedValue(undefined as never);
    m.redoRuleEntry.mockResolvedValue(undefined);
  });

  it('groups entries by category and filters by search', async () => {
    m.getRuleEntries.mockResolvedValue([
      rule('r1', 'Initiative', 'mechanic'),
      rule('r2', 'Fireball', 'ability'),
    ]);
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await waitFor(() => expect(screen.getByText('Initiative')).toBeTruthy());
    expect(screen.getByRole('heading', { name: /mechanic/i })).toBeTruthy();
    await fireEvent.input(screen.getByLabelText('Search rules'), { target: { value: 'fire' } });
    await waitFor(() => expect(screen.queryByText('Initiative')).toBeNull());
    expect(screen.getByText('Fireball')).toBeTruthy();
  });

  it('saves table notes on blur and submits objections', async () => {
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    m.updateRuleNotes.mockResolvedValueOnce({
      ...rule('r1', 'Initiative', 'mechanic'),
      notes: 'we roll once per round',
    } as never);
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await fireEvent.click(await screen.findByText('Initiative'));
    const notes = screen.getByLabelText('Table notes');
    await fireEvent.input(notes, { target: { value: 'we roll once per round' } });
    await fireEvent.blur(notes);
    await waitFor(() =>
      expect(m.updateRuleNotes).toHaveBeenCalledWith('r1', 'we roll once per round'),
    );
    await fireEvent.click(screen.getByText(/Redo with objections/));
    await fireEvent.input(screen.getByLabelText('Objection'), {
      target: { value: 'range is wrong' },
    });
    await fireEvent.click(screen.getByText('Submit'));
    await waitFor(() => expect(m.redoRuleEntry).toHaveBeenCalledWith('r1', 'range is wrong'));
  });

  it('shows an error message when loading entries fails, not the empty state', async () => {
    m.getRuleEntries.mockRejectedValue(new Error('boom'));
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy());
    expect(screen.getByText(/Failed to load rule entries/)).toBeTruthy();
    expect(screen.queryByText('No rule entries compiled yet.')).toBeNull();
  });

  it('does not save notes on blur when the draft is unchanged', async () => {
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await fireEvent.click(await screen.findByText('Initiative'));
    const notes = screen.getByLabelText('Table notes');
    await fireEvent.blur(notes);
    expect(m.updateRuleNotes).not.toHaveBeenCalled();
  });

  it('keeps the notes baseline fresh after a save, so clearing then blurring saves null', async () => {
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    m.updateRuleNotes
      .mockResolvedValueOnce({
        ...rule('r1', 'Initiative', 'mechanic'),
        notes: 'house rule',
      } as never)
      .mockResolvedValueOnce(rule('r1', 'Initiative', 'mechanic') as never);
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await fireEvent.click(await screen.findByText('Initiative'));
    const notes = screen.getByLabelText('Table notes');

    await fireEvent.input(notes, { target: { value: 'house rule' } });
    await fireEvent.blur(notes);
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledWith('r1', 'house rule'));

    await fireEvent.input(notes, { target: { value: '' } });
    await fireEvent.blur(notes);
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledWith('r1', null));
    expect(m.updateRuleNotes).toHaveBeenCalledTimes(2);
  });

  it('collapses page refs to a single page when start equals end', async () => {
    m.getRuleEntries.mockResolvedValue([
      {
        ...rule('r1', 'Initiative', 'mechanic'),
        page_refs: [{ source_name: 'PHB', page_start: 42, page_end: 42 }],
      },
    ]);
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await fireEvent.click(await screen.findByText('Initiative'));
    expect(screen.getByText(/PHB p\.42/)).toBeTruthy();
    expect(screen.queryByText(/p\.42-42/)).toBeNull();
  });

  it('shows the redo error and keeps the dialog open when redo fails', async () => {
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    m.redoRuleEntry.mockRejectedValue(new Error('redo boom'));
    render(RulesPanel, { props: { collectionId: 'c-1' } });
    await fireEvent.click(await screen.findByText('Initiative'));
    await fireEvent.click(screen.getByText(/Redo with objections/));
    await fireEvent.input(screen.getByLabelText('Objection'), {
      target: { value: 'range is wrong' },
    });
    await fireEvent.click(screen.getByText('Submit'));
    await waitFor(() => expect(screen.getByRole('alert')).toBeTruthy());
    expect(screen.getByText(/Failed to redo: Error: redo boom/)).toBeTruthy();
    expect(screen.getByLabelText('Objection')).toBeTruthy();
  });

  it('announces pending, saving, and canonical acknowledged state for table notes', async () => {
    const save = deferred<RuleEntry>();
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    m.updateRuleNotes.mockReturnValue(save.promise as never);
    renderPanel();
    const notes = await openNotes();

    await fireEvent.input(notes, { target: { value: 'One roll per side.' } });
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
    await fireEvent.blur(notes);
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledOnce());
    expect(screen.getByRole('status')).toHaveTextContent('Saving…');

    save.resolve({ ...rule('r1', 'Initiative', 'mechanic'), notes: 'One roll per side.' });
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    expect(notes).toHaveValue('One roll per side.');
  });

  it('retains a rule draft across collapse and remount without leaking to another campaign', async () => {
    const coordinator = new DraftCoordinator();
    m.getRuleEntries.mockResolvedValue([
      { ...rule('r1', 'Initiative', 'mechanic'), notes: 'Saved rule note.' },
    ]);
    const first = renderPanel(coordinator);
    const notes = await openNotes();
    await fireEvent.input(notes, { target: { value: 'Campaign A local rule.' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Initiative' }));
    await fireEvent.click(screen.getByRole('button', { name: 'Initiative' }));
    expect(screen.getByRole('textbox', { name: 'Table notes' })).toHaveValue(
      'Campaign A local rule.',
    );
    first.unmount();

    const sameCampaign = renderPanel(coordinator);
    expect(await openNotes()).toHaveValue('Campaign A local rule.');
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
    sameCampaign.unmount();

    renderPanel(coordinator, { campaignId: 'camp-b' });
    expect(await openNotes()).toHaveValue('Saved rule note.');
    expect(screen.queryByText('Campaign A local rule.')).not.toBeInTheDocument();
  });

  it('retains only minimal presentation metadata once a rule note becomes at risk', async () => {
    const coordinator = new DraftCoordinator();
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    renderPanel(coordinator);
    const notes = await openNotes();

    expect(rememberedRuleRecovery(coordinator, ruleScope('camp-a', 'c-1', 'r1'))).toBeUndefined();
    await fireEvent.input(notes, { target: { value: 'A local ruling.' } });

    expect(rememberedRuleRecovery(coordinator, ruleScope('camp-a', 'c-1', 'r1'))).toEqual({
      ruleId: 'r1',
      title: 'Initiative',
      collectionId: 'c-1',
    });

    await fireEvent.input(notes, { target: { value: '' } });
    expect(rememberedRuleRecovery(coordinator, ruleScope('camp-a', 'c-1', 'r1'))).toBeUndefined();
  });

  it('releases clean rule projections when the panel unmounts', async () => {
    const coordinator = new DraftCoordinator();
    const scope = ruleScope('camp-a', 'c-1', 'r1');
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    const rendered = renderPanel(coordinator);

    await screen.findByRole('button', { name: 'Initiative' });
    expect(coordinator.get(scope)).toBeDefined();
    rendered.unmount();

    expect(coordinator.get(scope)).toBeUndefined();
  });

  it('releases the prior clean rule projection when the panel changes scope', async () => {
    const coordinator = new DraftCoordinator();
    const priorScope = ruleScope('camp-a', 'c-1', 'r1');
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    const rendered = renderPanel(coordinator);
    await screen.findByRole('button', { name: 'Initiative' });

    await rendered.rerender({
      campaignId: 'camp-b',
      collectionId: 'c-2',
      draftCoordinator: coordinator,
    } as never);

    await waitFor(() => expect(coordinator.get(priorScope)).toBeUndefined());
    expect(coordinator.get(ruleScope('camp-b', 'c-2', 'r1'))).toBeDefined();
  });

  it('reconciles preexisting clean omissions on the first load and retains at-risk ones', async () => {
    const coordinator = new DraftCoordinator();
    const cleanScope = ruleScope('camp-a', 'c-1', 'clean');
    const dirtyScope = ruleScope('camp-a', 'c-1', 'dirty');
    coordinator.open(
      cleanScope,
      'rule:clean',
      { notes: 'Saved clean note.' },
      'authoritative-list',
    );
    coordinator.open(
      dirtyScope,
      'rule:dirty',
      { notes: 'Saved dirty note.' },
      'authoritative-list',
    );
    coordinator.revise(dirtyScope, { notes: 'Retained local note.' });
    m.getRuleEntries.mockResolvedValue([]);

    renderPanel(coordinator);

    await waitFor(() => expect(coordinator.get(cleanScope)).toBeUndefined());
    expect(coordinator.get<{ notes: string }>(dirtyScope)?.value.notes).toBe(
      'Retained local note.',
    );
    const retained = coordinator.get(dirtyScope);
    if (!retained) throw new Error('Expected the at-risk omitted rule draft to remain retained');
    expect(statusOf(retained)).toBe('pending');
  });

  it('forgets recovery metadata after an unmounted save succeeds', async () => {
    const coordinator = new DraftCoordinator();
    const save = deferred<RuleEntry>();
    const scope = ruleScope('camp-a', 'c-1', 'r1');
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    m.updateRuleNotes.mockReturnValue(save.promise as never);
    const rendered = renderPanel(coordinator);
    const notes = await openNotes();
    await fireEvent.input(notes, { target: { value: 'Transient local ruling.' } });
    await fireEvent.blur(notes);
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledOnce());
    expect(rememberedRuleRecovery(coordinator, scope)).toBeDefined();

    rendered.unmount();
    save.resolve({ ...rule('r1', 'Initiative', 'mechanic'), notes: 'Transient local ruling.' });

    await waitFor(() => expect(coordinator.get(scope)).toBeUndefined());
    expect(rememberedRuleRecovery(coordinator, scope)).toBeUndefined();
  });

  it('keeps a newer note pending when an older acknowledgment equals it', async () => {
    const first = deferred<RuleEntry>();
    const second = deferred<RuleEntry>();
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    m.updateRuleNotes
      .mockReturnValueOnce(first.promise as never)
      .mockReturnValueOnce(second.promise as never);
    renderPanel();
    const notes = await openNotes();

    await fireEvent.input(notes, { target: { value: 'Revision one.' } });
    await fireEvent.blur(notes);
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledOnce());
    await fireEvent.input(notes, { target: { value: 'Revision two.' } });
    first.resolve({ ...rule('r1', 'Initiative', 'mechanic'), notes: 'Revision two.' });

    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes'));
    expect(notes).toHaveValue('Revision two.');
    expect(screen.queryByText('Saved', { exact: true })).not.toBeInTheDocument();

    await fireEvent.blur(notes);
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledTimes(2));
    second.resolve({ ...rule('r1', 'Initiative', 'mechanic'), notes: 'Revision two.' });
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
  });

  it('serializes rapid rule-note saves and coalesces to the newest note', async () => {
    const first = deferred<RuleEntry>();
    const latest = deferred<RuleEntry>();
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    m.updateRuleNotes
      .mockReturnValueOnce(first.promise as never)
      .mockReturnValueOnce(latest.promise as never);
    renderPanel();
    const notes = await openNotes();

    for (const value of ['First ruling.', 'Second ruling.', 'Newest ruling.']) {
      await fireEvent.input(notes, { target: { value } });
      await fireEvent.blur(notes);
    }
    expect(m.updateRuleNotes).toHaveBeenCalledTimes(1);

    first.resolve({ ...rule('r1', 'Initiative', 'mechanic'), notes: 'First ruling.' });
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledTimes(2));
    expect(m.updateRuleNotes.mock.calls[1]).toEqual(['r1', 'Newest ruling.']);
    expect(notes).toHaveValue('Newest ruling.');

    latest.resolve({ ...rule('r1', 'Initiative', 'mechanic'), notes: 'Newest ruling.' });
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
  });

  it('keeps a failed note actionable and returns focus to its textarea after successful Retry', async () => {
    const saved = { ...rule('r1', 'Initiative', 'mechanic'), notes: 'Retained house rule.' };
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    m.updateRuleNotes
      .mockRejectedValueOnce({ code: 'DATABASE', message: 'Rule-note write failed.' })
      .mockResolvedValueOnce(saved as never);
    renderPanel();
    const user = userEvent.setup();
    const notes = await openNotes();
    await fireEvent.input(notes, { target: { value: 'Retained house rule.' } });
    await fireEvent.blur(notes);

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('Rule-note write failed.');
    expect(notes).toHaveValue('Retained house rule.');
    const retry = screen.getByRole('button', { name: 'Retry' });
    retry.focus();
    await user.keyboard('{Enter}');

    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledTimes(2));
    expect(m.updateRuleNotes.mock.calls[1]).toEqual(['r1', 'Retained house rule.']);
    await waitFor(() => expect(screen.queryByRole('alert')).not.toBeInTheDocument());
    expect(notes).toHaveFocus();
    expect(screen.getByRole('status')).toHaveTextContent('Saved');
  });

  it('retains Retry focus and the original rule target when unavailable Retry fails again', async () => {
    const backendDetail = 'Internal rule_entry row r1 vanished during UPDATE.';
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    m.updateRuleNotes.mockRejectedValue({
      code: 'NOT_FOUND',
      message: backendDetail,
    });
    renderPanel();
    const user = userEvent.setup();
    const notes = await openNotes();
    await fireEvent.input(notes, { target: { value: 'Last local ruling.' } });
    await fireEvent.blur(notes);

    const retry = await screen.findByRole('button', { name: 'Retry' });
    expect(screen.getByRole('alert')).toHaveTextContent(i18n.t('drafts.targetUnavailable'));
    expect(screen.getByRole('alert')).not.toHaveTextContent(backendDetail);
    retry.focus();
    await user.keyboard('{Enter}');

    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledTimes(2));
    expect(m.updateRuleNotes.mock.calls[1]).toEqual(['r1', 'Last local ruling.']);
    expect(notes).toHaveValue('Last local ruling.');
    expect(screen.getByRole('button', { name: 'Retry' })).toHaveFocus();
  });

  it('keeps drafts separate when switching between rule records', async () => {
    m.getRuleEntries.mockResolvedValue([
      { ...rule('r1', 'Initiative', 'mechanic'), notes: 'Initiative saved.' },
      { ...rule('r2', 'Surprise', 'mechanic'), notes: 'Surprise saved.' },
    ]);
    renderPanel();
    const initiative = await openNotes('Initiative');
    await fireEvent.input(initiative, { target: { value: 'Initiative local.' } });
    await fireEvent.click(screen.getByRole('button', { name: 'Surprise' }));
    expect(screen.getByRole('textbox', { name: 'Table notes' })).toHaveValue('Surprise saved.');
    await fireEvent.click(screen.getByRole('button', { name: 'Initiative' }));
    expect(screen.getByRole('textbox', { name: 'Table notes' })).toHaveValue('Initiative local.');
  });

  it('separates identical rule IDs by collection and the explicit no-campaign context', async () => {
    const coordinator = new DraftCoordinator();
    m.getRuleEntries.mockResolvedValue([
      { ...rule('r1', 'Initiative', 'mechanic'), notes: 'Shared saved note.' },
    ]);
    const campaignCollection = renderPanel(coordinator);
    await fireEvent.input(await openNotes(), { target: { value: 'Campaign A, collection one.' } });
    campaignCollection.unmount();

    const otherCollection = renderPanel(coordinator, { collectionId: 'c-2' });
    expect(await openNotes()).toHaveValue('Shared saved note.');
    otherCollection.unmount();

    const noCampaign = renderPanel(coordinator, { campaignId: null });
    const noCampaignNotes = await openNotes();
    expect(noCampaignNotes).toHaveValue('Shared saved note.');
    await fireEvent.input(noCampaignNotes, { target: { value: 'No-campaign local note.' } });
    noCampaign.unmount();

    renderPanel(coordinator);
    expect(await openNotes()).toHaveValue('Campaign A, collection one.');
    expect(screen.queryByText('No-campaign local note.')).not.toBeInTheDocument();
  });

  it('retains a focused note without saving when focus leaves its editing scope', async () => {
    const coordinator = new DraftCoordinator();
    m.getRuleEntries.mockResolvedValue([
      { ...rule('r1', 'Initiative', 'mechanic'), notes: 'Saved rule note.' },
    ]);
    const first = renderPanel(coordinator);
    const notes = await openNotes();
    await fireEvent.input(notes, { target: { value: 'Retained during navigation.' } });

    const navigation = document.createElement('button');
    navigation.type = 'button';
    navigation.textContent = 'Open campaign notebook';
    document.body.append(navigation);
    try {
      await fireEvent.blur(notes, { relatedTarget: navigation });
      navigation.focus();
      expect(m.updateRuleNotes).not.toHaveBeenCalled();

      first.unmount();
      renderPanel(coordinator);
      expect(await openNotes()).toHaveValue('Retained during navigation.');
      expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
    } finally {
      navigation.remove();
    }
  });

  it('does not autosave when the close decision moves focus without a related target', async () => {
    const coordinator = new DraftCoordinator();
    m.getRuleEntries.mockResolvedValue([
      { ...rule('r1', 'Initiative', 'mechanic'), notes: 'Saved rule note.' },
    ]);
    renderPanel(coordinator);
    const notes = await openNotes();
    await fireEvent.input(notes, { target: { value: 'Retained for close decision.' } });

    coordinator.beginCloseDecision();
    await fireEvent.blur(notes, { relatedTarget: null });

    expect(m.updateRuleNotes).not.toHaveBeenCalled();
    expect(notes).toHaveValue('Retained for close decision.');
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
  });

  it('clears pending without IPC when a null-backed note returns to its saved baseline', async () => {
    m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
    renderPanel();
    const notes = await openNotes();
    expect(notes).toHaveValue('');

    await fireEvent.input(notes, { target: { value: 'Temporary ruling.' } });
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
    await fireEvent.input(notes, { target: { value: '' } });
    await fireEvent.blur(notes);

    expect(m.updateRuleNotes).not.toHaveBeenCalled();
    expect(screen.queryByText('Unsaved changes', { exact: true })).not.toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('Saved');
  });

  it('discards with the keyboard without blur-saving or changing an unrelated rule draft', async () => {
    const coordinator = new DraftCoordinator();
    const unrelatedScope = ruleScope('camp-a', 'c-1', 'r2');
    coordinator.open(unrelatedScope, 'rule:r2', { notes: 'Surprise saved.' }, 'authoritative-list');
    coordinator.revise(unrelatedScope, { notes: 'Unrelated retained ruling.' });
    m.getRuleEntries.mockResolvedValue([
      { ...rule('r1', 'Initiative', 'mechanic'), notes: 'Initiative saved.' },
      { ...rule('r2', 'Surprise', 'mechanic'), notes: 'Surprise saved.' },
    ]);
    renderPanel(coordinator);
    const user = userEvent.setup();
    const notes = await openNotes();
    await user.clear(notes);
    await user.type(notes, 'Discard this local ruling.');

    const entryHeader = screen.getByRole('button', { name: 'Initiative' });
    const entryItem = entryHeader.closest('li');
    if (!entryItem) throw new Error('Expected Initiative to be rendered in a rule-entry row');
    const discard = within(entryItem).getByRole('button', { name: 'Discard changes' });
    discard.focus();
    await user.keyboard('{Enter}');

    await waitFor(() => expect(notes).toHaveValue('Initiative saved.'));
    expect(m.updateRuleNotes).not.toHaveBeenCalled();
    expect(entryHeader).toHaveFocus();
    expect(coordinator.get<{ notes: string }>(unrelatedScope)?.value.notes).toBe(
      'Unrelated retained ruling.',
    );
    const unrelatedDraft = coordinator.get<{ notes: string }>(unrelatedScope);
    if (!unrelatedDraft) throw new Error('Expected the unrelated rule draft to remain open');
    expect(statusOf(unrelatedDraft)).toBe('pending');
  });

  it.each([
    {
      locale: 'en' as const,
      unavailable: 'This record is no longer available.',
      retryName: 'Retry',
    },
    {
      locale: 'de' as const,
      unavailable: 'Dieser Eintrag ist nicht mehr verfügbar.',
      retryName: 'Erneut versuchen',
    },
  ])(
    'localizes an unavailable rule-note failure in $locale and retries its exact target',
    async ({ locale, unavailable, retryName }) => {
      const backendDetail = 'Internal rule_entry row 73 vanished during UPDATE.';
      i18n.setLocale(locale);
      m.getRuleEntries.mockResolvedValue([rule('r1', 'Initiative', 'mechanic')]);
      m.updateRuleNotes.mockRejectedValue({ code: 'NOT_FOUND', message: backendDetail });
      renderPanel();
      const user = userEvent.setup();
      await fireEvent.click(await screen.findByRole('button', { name: 'Initiative' }));
      const notes = screen.getByRole('textbox', { name: i18n.t('entityUi.tableNotes') });
      await fireEvent.input(notes, { target: { value: 'Retain this exact table ruling.' } });
      await fireEvent.blur(notes);

      const alert = await screen.findByRole('alert');
      expect(alert).toHaveTextContent(unavailable);
      expect(alert).not.toHaveTextContent(backendDetail);
      const retry = within(alert).getByRole('button', { name: retryName });
      retry.focus();
      await user.keyboard('{Enter}');

      await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledTimes(2));
      expect(m.updateRuleNotes.mock.calls).toEqual([
        ['r1', 'Retain this exact table ruling.'],
        ['r1', 'Retain this exact table ruling.'],
      ]);
      expect(notes).toHaveValue('Retain this exact table ruling.');
      expect(
        within(screen.getByRole('alert')).getByRole('button', { name: retryName }),
      ).toHaveFocus();
    },
  );

  it('rejects a wrong-rule acknowledgment and retries the original rule and content', async () => {
    const coordinator = new DraftCoordinator();
    const initiative = { ...rule('r1', 'Initiative', 'mechanic'), notes: 'Initiative saved.' };
    const surprise = { ...rule('r2', 'Surprise', 'mechanic'), notes: 'Surprise saved.' };
    m.getRuleEntries.mockResolvedValue([initiative, surprise]);
    m.updateRuleNotes
      .mockResolvedValueOnce({ ...surprise, notes: 'Wrong canonical content.' } as never)
      .mockResolvedValueOnce({ ...initiative, notes: 'Retained Initiative ruling.' } as never);
    renderPanel(coordinator);
    const notes = await openNotes();
    await fireEvent.input(notes, { target: { value: 'Retained Initiative ruling.' } });
    await fireEvent.blur(notes);

    const failure = await screen.findByRole('alert');
    expect(failure).toHaveTextContent("Couldn't save");
    expect(notes).toHaveValue('Retained Initiative ruling.');
    const initiativeDraft = coordinator.get(ruleScope('camp-a', 'c-1', 'r1'));
    if (!initiativeDraft) throw new Error('Expected the Initiative draft to remain open');
    expect(statusOf(initiativeDraft)).toBe('failed');
    expect(coordinator.get<{ notes: string }>(ruleScope('camp-a', 'c-1', 'r2'))?.value.notes).toBe(
      'Surprise saved.',
    );

    await fireEvent.click(within(failure).getByRole('button', { name: 'Retry' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Saved'));
    expect(m.updateRuleNotes.mock.calls).toEqual([
      ['r1', 'Retained Initiative ruling.'],
      ['r1', 'Retained Initiative ruling.'],
    ]);
    expect(notes).toHaveValue('Retained Initiative ruling.');
    expect(coordinator.get<{ notes: string }>(ruleScope('camp-a', 'c-1', 'r2'))?.value.notes).toBe(
      'Surprise saved.',
    );
  });

  it('keeps a missing failed rule recoverable and discards only that retained draft', async () => {
    const coordinator = new DraftCoordinator();
    const initiativeScope = ruleScope('camp-a', 'c-1', 'r1');
    const surpriseScope = ruleScope('camp-a', 'c-1', 'r2');
    coordinator.open(surpriseScope, 'rule:r2', { notes: 'Surprise saved.' }, 'authoritative-list');
    coordinator.revise(surpriseScope, { notes: 'Unrelated retained surprise ruling.' });
    m.getRuleEntries
      .mockResolvedValueOnce([
        { ...rule('r1', 'Initiative', 'mechanic'), notes: 'Initiative saved.' },
        { ...rule('r2', 'Surprise', 'mechanic'), notes: 'Surprise saved.' },
      ])
      .mockResolvedValueOnce([{ ...rule('r2', 'Surprise', 'mechanic'), notes: 'Surprise saved.' }]);
    m.updateRuleNotes.mockRejectedValue({
      code: 'NOT_FOUND',
      message: 'Internal deleted rule identifier r1.',
    });

    const first = renderPanel(coordinator);
    const notes = await openNotes();
    await fireEvent.input(notes, { target: { value: 'Evidence retained after deletion.' } });
    await fireEvent.blur(notes);
    await screen.findByRole('alert');
    first.unmount();

    renderPanel(coordinator);
    const unavailableHeader = await screen.findByRole('button', { name: 'Initiative' });
    expect(unavailableHeader).toHaveAccessibleDescription(/couldn't save/i);
    await fireEvent.click(unavailableHeader);
    expect(screen.getByRole('textbox', { name: 'Table notes' })).toHaveValue(
      'Evidence retained after deletion.',
    );
    const failure = await screen.findByRole('alert');
    expect(within(failure).getByRole('button', { name: 'Retry' })).toBeEnabled();
    await fireEvent.click(within(failure).getByRole('button', { name: 'Discard changes' }));

    await waitFor(() => {
      expect(screen.queryByRole('button', { name: 'Initiative' })).not.toBeInTheDocument();
    });
    expect(coordinator.get(initiativeScope)).toBeUndefined();
    expect(rememberedRuleRecovery(coordinator, initiativeScope)).toBeUndefined();
    expect(coordinator.get<{ notes: string }>(surpriseScope)?.value.notes).toBe(
      'Unrelated retained surprise ruling.',
    );
    const retainedSurprise = coordinator.get<{ notes: string }>(surpriseScope);
    if (!retainedSurprise) throw new Error('Expected the unrelated rule draft to remain open');
    expect(statusOf(retainedSurprise)).toBe('pending');
  });

  it('rejects pre-ack and obsolete unmounted list authority over a newer rule-note revision', async () => {
    const coordinator = new DraftCoordinator();
    const save = deferred<RuleEntry>();
    const preAcknowledgmentList = deferred<RuleEntry[]>();
    const obsoleteUnmountedList = deferred<RuleEntry[]>();
    const baseline = { ...rule('r1', 'Initiative', 'mechanic'), notes: 'Original saved note.' };
    const acknowledged = {
      ...baseline,
      notes: 'Canonical acknowledgment for revision one.',
    };
    m.getRuleEntries
      .mockResolvedValueOnce([baseline])
      .mockReturnValueOnce(preAcknowledgmentList.promise)
      .mockReturnValueOnce(obsoleteUnmountedList.promise)
      .mockResolvedValueOnce([acknowledged]);
    m.updateRuleNotes.mockReturnValue(save.promise as never);

    const first = renderPanel(coordinator);
    const firstNotes = await openNotes();
    await fireEvent.input(firstNotes, { target: { value: 'Revision one.' } });
    await fireEvent.blur(firstNotes);
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledOnce());
    await fireEvent.input(firstNotes, { target: { value: 'Newer retained revision.' } });
    first.unmount();

    const preAckInstance = renderPanel(coordinator);
    await waitFor(() => expect(m.getRuleEntries).toHaveBeenCalledTimes(2));
    save.resolve(acknowledged);
    preAcknowledgmentList.resolve([baseline]);
    const retainedAfterPreAck = await openNotes();
    expect(retainedAfterPreAck).toHaveValue('Newer retained revision.');
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
    expect(screen.queryByText('Saved', { exact: true })).not.toBeInTheDocument();
    preAckInstance.unmount();

    const obsoleteInstance = renderPanel(coordinator);
    await waitFor(() => expect(m.getRuleEntries).toHaveBeenCalledTimes(3));
    obsoleteInstance.unmount();
    renderPanel(coordinator);
    await waitFor(() => expect(m.getRuleEntries).toHaveBeenCalledTimes(4));
    obsoleteUnmountedList.resolve([{ ...baseline, notes: 'Newer retained revision.' }]);
    await obsoleteUnmountedList.promise;
    await tick();

    expect(await openNotes()).toHaveValue('Newer retained revision.');
    expect(screen.getByRole('status')).toHaveTextContent('Unsaved changes');
    const latestDraft = coordinator.get<{ notes: string }>(ruleScope('camp-a', 'c-1', 'r1'));
    if (!latestDraft) throw new Error('Expected the retained rule draft to remain open');
    expect(statusOf(latestDraft)).toBe('pending');
  });

  it('keeps compact pending, saving, and failed state visible on a collapsed rule row', async () => {
    const save = deferred<RuleEntry>();
    m.getRuleEntries.mockResolvedValue([
      { ...rule('r1', 'Initiative', 'mechanic'), notes: 'Saved initiative ruling.' },
    ]);
    m.updateRuleNotes.mockReturnValue(save.promise as never);
    renderPanel();
    const notes = await openNotes();
    await fireEvent.input(notes, { target: { value: 'Retained collapsed ruling.' } });

    const header = screen.getByRole('button', { name: 'Initiative' });
    const row = header.closest('li');
    if (!row) throw new Error('Expected Initiative to be rendered in a rule-entry row');
    await fireEvent.click(header);
    expect(within(row).getByText(i18n.t('drafts.unsavedChanges'), { exact: true })).toBeVisible();
    expect(header).toHaveAccessibleDescription(i18n.t('drafts.unsavedChanges'));

    await fireEvent.click(header);
    const expandedNotes = screen.getByRole('textbox', { name: i18n.t('entityUi.tableNotes') });
    const sameScopeControl = screen.getByRole('button', {
      name: i18n.t('entityUi.redoWithObjections'),
    });
    await fireEvent.blur(expandedNotes, { relatedTarget: sameScopeControl });
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledOnce());
    await fireEvent.click(header);
    expect(within(row).getByText(i18n.t('drafts.saving'), { exact: true })).toBeVisible();
    expect(header).toHaveAccessibleDescription(i18n.t('drafts.saving'));

    save.reject({ code: 'DATABASE', message: 'Offline while saving the ruling.' });
    await waitFor(() => {
      expect(within(row).getByText(i18n.t('drafts.couldNotSave'), { exact: true })).toBeVisible();
    });
    expect(header).toHaveAccessibleDescription(i18n.t('drafts.couldNotSave'));
    expect(within(row).queryByRole('button', { name: i18n.t('drafts.retry') })).toBeNull();
  });

  it('does not move focus into a new context when an older Retry completes', async () => {
    const retrySave = deferred<RuleEntry>();
    const campaignARule = {
      ...rule('r1', 'Initiative', 'mechanic'),
      notes: 'Campaign A saved ruling.',
    };
    const campaignBRule = {
      ...rule('r1', 'Initiative', 'mechanic'),
      notes: 'Campaign B saved ruling.',
    };
    m.getRuleEntries.mockImplementation(async (collectionId) =>
      collectionId === 'c-1' ? [campaignARule] : [campaignBRule],
    );
    m.updateRuleNotes
      .mockRejectedValueOnce({ code: 'DATABASE', message: 'Retry this ruling.' })
      .mockReturnValueOnce(retrySave.promise as never);
    const coordinator = new DraftCoordinator();
    const rendered = renderPanel(coordinator);
    const notes = await openNotes();
    await fireEvent.input(notes, { target: { value: 'Campaign A retry content.' } });
    await fireEvent.blur(notes);
    const retry = await screen.findByRole('button', { name: i18n.t('drafts.retry') });
    await fireEvent.click(retry);
    await waitFor(() => expect(m.updateRuleNotes).toHaveBeenCalledTimes(2));

    await rendered.rerender({
      campaignId: 'camp-b',
      collectionId: 'c-2',
      draftCoordinator: coordinator,
    } as never);
    await waitFor(() => expect(m.getRuleEntries).toHaveBeenCalledWith('c-2'));
    const campaignBNotes = await screen.findByRole('textbox', {
      name: i18n.t('entityUi.tableNotes'),
    });
    expect(campaignBNotes).toHaveValue('Campaign B saved ruling.');
    const campaignBFocusTarget = screen.getByRole('textbox', {
      name: i18n.t('entityUi.searchRules'),
    });
    campaignBFocusTarget.focus();

    retrySave.resolve({ ...campaignARule, notes: 'Campaign A retry content.' });
    await waitFor(() => {
      const campaignADraft = coordinator.get<{ notes: string }>(ruleScope('camp-a', 'c-1', 'r1'));
      expect(campaignADraft).toBeUndefined();
    });
    expect(campaignBFocusTarget).toHaveFocus();
    expect(campaignBNotes).toHaveValue('Campaign B saved ruling.');
    const campaignBDraft = coordinator.get<{ notes: string }>(ruleScope('camp-b', 'c-2', 'r1'));
    if (!campaignBDraft) throw new Error('Expected Campaign B rule draft to remain open');
    expect(campaignBDraft.lastAcknowledgedAttemptId).toBe(0);
    expect(campaignBDraft.value.notes).toBe('Campaign B saved ruling.');
  });
});
