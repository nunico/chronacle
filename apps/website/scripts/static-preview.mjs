import { readFile, realpath, stat } from 'node:fs/promises';
import { createServer } from 'node:http';
import { extname, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const buildDirectory = fileURLToPath(new URL('../build/', import.meta.url));
const contentTypes = new Map([
  ['.css', 'text/css; charset=utf-8'],
  ['.html', 'text/html; charset=utf-8'],
  ['.js', 'text/javascript; charset=utf-8'],
  ['.json', 'application/json; charset=utf-8'],
  ['.mjs', 'text/javascript; charset=utf-8'],
  ['.pdf', 'application/pdf'],
  ['.png', 'image/png'],
  ['.svg', 'image/svg+xml'],
  ['.txt', 'text/plain; charset=utf-8'],
  ['.wasm', 'application/wasm'],
  ['.woff', 'font/woff'],
  ['.woff2', 'font/woff2'],
]);

function isInsideRoot(rootDirectory, candidate) {
  const pathFromRoot = relative(rootDirectory, candidate);
  return pathFromRoot === '' || (!pathFromRoot.startsWith(`..${sep}`) && pathFromRoot !== '..');
}

async function existingFile(realRootDirectory, candidate) {
  try {
    const realCandidate = await realpath(candidate);
    if (!isInsideRoot(realRootDirectory, realCandidate)) return undefined;

    const details = await stat(realCandidate);
    if (details.isFile()) return realCandidate;
    if (details.isDirectory()) {
      const indexFile = await realpath(resolve(realCandidate, 'index.html'));
      if (!isInsideRoot(realRootDirectory, indexFile)) return undefined;
      return (await stat(indexFile)).isFile() ? indexFile : undefined;
    }
  } catch {
    return undefined;
  }
  return undefined;
}

async function resolveFile(rootDirectory, realRootDirectory, pathname) {
  const decodedPathname = decodeURIComponent(pathname);
  const candidate = resolve(rootDirectory, decodedPathname.replace(/^\/+/, ''));
  if (!isInsideRoot(rootDirectory, candidate)) return { invalid: true };

  const directFile = await existingFile(realRootDirectory, candidate);
  if (directFile) return { file: directFile };

  if (extname(candidate) === '') {
    const htmlFile = await existingFile(realRootDirectory, `${candidate}.html`);
    if (htmlFile) return { file: htmlFile };
  }

  return {};
}

async function sendFile(response, file, statusCode, headOnly) {
  const body = await readFile(file);
  response.writeHead(statusCode, {
    'cache-control': 'no-cache',
    'content-length': body.byteLength,
    'content-type': contentTypes.get(extname(file)) ?? 'application/octet-stream',
  });
  response.end(headOnly ? undefined : body);
}

export function createStaticServer(rootDirectory = buildDirectory) {
  const resolvedRootDirectory = resolve(rootDirectory);
  let rootConfiguration;
  const getRootConfiguration = () => {
    rootConfiguration ??= realpath(resolvedRootDirectory).then(async (realRootDirectory) => {
      const fallbackFile = await existingFile(
        realRootDirectory,
        resolve(resolvedRootDirectory, '404.html'),
      );
      if (!fallbackFile) {
        throw new Error('Static fallback is outside the build directory or missing');
      }
      return { fallbackFile, realRootDirectory };
    });
    return rootConfiguration;
  };

  return createServer(async (request, response) => {
    const method = request.method ?? 'GET';
    if (method !== 'GET' && method !== 'HEAD') {
      response.writeHead(405, { allow: 'GET, HEAD', 'content-type': 'text/plain; charset=utf-8' });
      response.end('Method not allowed');
      return;
    }

    try {
      const requestUrl = new URL(request.url ?? '/', 'http://static-preview.invalid');
      const { fallbackFile, realRootDirectory } = await getRootConfiguration();
      const resolved = await resolveFile(
        resolvedRootDirectory,
        realRootDirectory,
        requestUrl.pathname,
      );
      if (resolved.invalid) {
        response.writeHead(400, { 'content-type': 'text/plain; charset=utf-8' });
        response.end('Invalid path');
        return;
      }
      await sendFile(
        response,
        resolved.file ?? fallbackFile,
        resolved.file ? 200 : 404,
        method === 'HEAD',
      );
    } catch (error) {
      if (error instanceof URIError) {
        response.writeHead(400, { 'content-type': 'text/plain; charset=utf-8' });
        response.end('Invalid path');
        return;
      }
      response.writeHead(500, { 'content-type': 'text/plain; charset=utf-8' });
      response.end('Static preview failed');
    }
  });
}

export function parseOptions(argumentsToParse) {
  let host = '127.0.0.1';
  let port = 4174;

  for (let index = 0; index < argumentsToParse.length; index += 1) {
    const argument = argumentsToParse[index];
    const [flag, inlineValue] = argument.split('=', 2);
    if (flag !== '--host' && flag !== '--port') continue;
    const value = inlineValue ?? argumentsToParse[++index];
    if (!value || value.startsWith('--')) throw new Error(`${flag} requires a value`);
    if (flag === '--host') {
      host = value;
    } else {
      port = Number(value);
      if (!Number.isInteger(port) || port < 1 || port > 65_535) {
        throw new Error('--port must be an integer from 1 to 65535');
      }
    }
  }

  return { host, port };
}

const isEntrypoint = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isEntrypoint) {
  try {
    const { host, port } = parseOptions(process.argv.slice(2));
    const server = createStaticServer();
    server.listen(port, host, () => {
      console.log(`Static preview listening on http://${host}:${port}`);
    });
    for (const signal of ['SIGINT', 'SIGTERM']) {
      process.once(signal, () => server.close(() => process.exit(0)));
    }
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
