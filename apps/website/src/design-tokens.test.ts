import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const appCss = readFileSync('src/app.css', 'utf8');
const buttonLink = readFileSync('src/lib/components/ButtonLink.svelte', 'utf8');
const siteHeader = readFileSync('src/lib/components/SiteHeader.svelte', 'utf8');
const productExample = readFileSync('src/lib/landing/ProductExample.svelte', 'utf8');

describe('Chronacle gradient tokens', () => {
  it('keeps the approved arcane gradient and uses a darker action gradient for primary controls', () => {
    expect(appCss).toContain(
      '--grad-arcane: linear-gradient(135deg, var(--arcane-500), var(--violet-500));',
    );
    expect(appCss).toContain(
      '--grad-action: linear-gradient(135deg, var(--arcane-500), var(--violet-600));',
    );
    expect(buttonLink).toContain('background: var(--grad-action);');
  });

  it('keeps outline button hover states free of arcane glow', () => {
    const outlineHoverRule = buttonLink.match(/\.button-link--outline:hover\s*\{([^}]*)\}/)?.[1];

    expect(outlineHoverRule).toBeDefined();
    expect(outlineHoverRule).not.toContain('box-shadow');
  });

  it('disables EyeMark glow at landing call sites', () => {
    expect(siteHeader).toContain('<EyeMark size={38} glow={false} />');
    expect(productExample).toContain('<EyeMark size={24} glow={false} />');
  });
});
