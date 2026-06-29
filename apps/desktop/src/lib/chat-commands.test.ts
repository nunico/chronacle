import { describe, it, expect } from 'vitest';
import { parseCommand } from './chat-commands';

describe('parseCommand', () => {
  it('parses /extract with a name', () => {
    expect(parseCommand('/extract Commander Varn')).toEqual({
      kind: 'extract',
      name: 'Commander Varn',
    });
  });

  it('trims surrounding whitespace from the name', () => {
    expect(parseCommand('  /extract   Iron Fist  ')).toEqual({
      kind: 'extract',
      name: 'Iron Fist',
    });
  });

  it('treats bare /extract as a usage hint, not a sweep', () => {
    expect(parseCommand('/extract')).toEqual({ kind: 'extract-usage' });
    expect(parseCommand('/extract   ')).toEqual({ kind: 'extract-usage' });
  });

  it('parses /extract-all', () => {
    expect(parseCommand('/extract-all')).toEqual({ kind: 'extract-all' });
  });

  it('parses /help', () => {
    expect(parseCommand('/help')).toEqual({ kind: 'help' });
  });

  it('treats unknown slash commands as help', () => {
    expect(parseCommand('/wat')).toEqual({ kind: 'help' });
  });

  it('passes normal text through as chat', () => {
    expect(parseCommand('How does grappling work?')).toEqual({
      kind: 'chat',
      text: 'How does grappling work?',
    });
  });

  it('does not treat a mid-sentence slash as a command', () => {
    expect(parseCommand('damage is 1/2')).toEqual({ kind: 'chat', text: 'damage is 1/2' });
  });
});
