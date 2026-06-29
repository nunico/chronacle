export type ChatCommand =
  | { kind: 'extract'; name: string }
  | { kind: 'extract-all' }
  | { kind: 'extract-usage' }
  | { kind: 'help' }
  | { kind: 'chat'; text: string };

/**
 * Classify chat input. Only a leading slash on the first non-space character
 * is treated as a command, so "1/2" stays normal chat.
 */
export function parseCommand(raw: string): ChatCommand {
  const text = raw.trim();
  if (!text.startsWith('/')) {
    return { kind: 'chat', text };
  }

  const spaceIdx = text.indexOf(' ');
  const head = (spaceIdx === -1 ? text : text.slice(0, spaceIdx)).toLowerCase();
  const rest = spaceIdx === -1 ? '' : text.slice(spaceIdx + 1).trim();

  switch (head) {
    case '/extract':
      return rest ? { kind: 'extract', name: rest } : { kind: 'extract-usage' };
    case '/extract-all':
      return { kind: 'extract-all' };
    case '/help':
      return { kind: 'help' };
    default:
      return { kind: 'help' };
  }
}
