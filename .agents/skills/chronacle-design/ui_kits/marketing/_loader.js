/* In-browser Svelte 5 loader.
 *
 * Fetches .svelte / .js sources, compiles Svelte components to client JS with the
 * official Svelte 5 compiler, rewrites relative imports to blob URLs, and mounts
 * the root component. Bare `svelte/*` specifiers resolve via the page's importmap.
 *
 * No build step — everything compiles in the browser. */
import { compile } from "https://esm.sh/svelte@5/compiler";
import { mount } from "svelte";

const cache = new Map(); // absolute source URL -> Promise<blob URL>

const resolve = (spec, fromUrl) => new URL(spec, fromUrl).href;

async function loadModule(url) {
  if (cache.has(url)) return cache.get(url);
  const p = (async () => {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`Failed to fetch ${url}: ${res.status}`);
    let code = await res.text();

    if (url.endsWith(".svelte")) {
      const out = compile(code, {
        generate: "client",
        dev: false,
        runes: true,
        css: "injected",
        filename: url.split("/").pop()
      });
      code = out.js.code;
    }

    code = await rewriteImports(code, url);
    const blob = new Blob([code], { type: "text/javascript" });
    return URL.createObjectURL(blob);
  })();
  cache.set(url, p);
  return p;
}

async function rewriteImports(code, fromUrl) {
  // Collect every relative specifier used in import/export ... from '...' or bare import '...'.
  const re = /(?:import|export)\b[^'"]*?(['"])(\.\.?\/[^'"]+)\1/g;
  const specs = new Set();
  let m;
  while ((m = re.exec(code)) !== null) specs.add(m[2]);

  for (const spec of specs) {
    const blobUrl = await loadModule(resolve(spec, fromUrl));
    code = code.split(`'${spec}'`).join(`'${blobUrl}'`).split(`"${spec}"`).join(`"${blobUrl}"`);
  }
  return code;
}

export async function boot(rootRelativePath, targetSelector = "#root") {
  const base = new URL(".", import.meta.url).href;
  try {
    const rootUrl = await loadModule(resolve(rootRelativePath, base));
    const mod = await import(rootUrl);
    mount(mod.default, { target: document.querySelector(targetSelector) });
  } catch (err) {
    console.error("[svelte-loader] boot failed:", err);
    const t = document.querySelector(targetSelector);
    if (t) {
      t.innerHTML =
        '<pre style="color:#ff9a9a;font:13px/1.5 ui-monospace,monospace;padding:24px;white-space:pre-wrap">' +
        String(err && err.stack ? err.stack : err) +
        "</pre>";
    }
  }
}
