/**
 * Collect Markdown links during mdsvex's existing parse pass and expose them
 * through the generated module metadata.
 *
 * @returns {(tree: unknown, file: { data: Record<string, unknown> }) => void}
 */
export function collectManualLinks() {
  return (tree, file) => {
    /** @type {Map<string, string>} */
    const definitions = new Map();
    /** @type {string[]} */
    const links = [];

    walk(tree, (node) => {
      if (
        node.type === 'definition' &&
        typeof node.identifier === 'string' &&
        typeof node.url === 'string'
      ) {
        definitions.set(node.identifier.toLowerCase(), node.url);
      }
    });

    walk(tree, (node) => {
      if (node.type === 'link' && typeof node.url === 'string') {
        links.push(node.url);
      }
      if (node.type === 'linkReference' && typeof node.identifier === 'string') {
        const url = definitions.get(node.identifier.toLowerCase());
        if (url !== undefined) {
          links.push(url);
        }
      }
    });

    const frontmatter = file.data.fm;
    if (isRecord(frontmatter)) {
      frontmatter.manualLinks = links;
    }
  };
}

/**
 * @param {unknown} value
 * @returns {value is Record<string, unknown>}
 */
function isRecord(value) {
  return typeof value === 'object' && value !== null;
}

/**
 * @param {unknown} value
 * @param {(node: Record<string, unknown>) => void} visitor
 */
function walk(value, visitor) {
  if (!isRecord(value)) {
    return;
  }

  visitor(value);
  if (!Array.isArray(value.children)) {
    return;
  }
  for (const child of value.children) {
    walk(child, visitor);
  }
}
