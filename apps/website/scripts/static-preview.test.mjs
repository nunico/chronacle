import assert from 'node:assert/strict';
import { mkdir, mkdtemp, rm, symlink, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, test } from 'node:test';
import { createStaticServer } from './static-preview.mjs';

const temporaryDirectories = [];

afterEach(async () => {
  await Promise.all(
    temporaryDirectories.splice(0).map((directory) => rm(directory, { recursive: true })),
  );
});

async function createFixture() {
  const workspace = await mkdtemp(join(tmpdir(), 'chronacle-static-preview-'));
  temporaryDirectories.push(workspace);
  const rootDirectory = join(workspace, 'build');
  const outsideDirectory = join(workspace, 'outside');
  await Promise.all([mkdir(rootDirectory), mkdir(outsideDirectory)]);
  await writeFile(join(rootDirectory, '404.html'), 'fallback fixture');
  return { outsideDirectory, rootDirectory };
}

async function createSymlinkOrSkip(testContext, target, path) {
  try {
    await symlink(target, path);
    return true;
  } catch (error) {
    if (
      error instanceof Error &&
      'code' in error &&
      ['EACCES', 'ENOSYS', 'EPERM'].includes(error.code)
    ) {
      testContext.skip(`symlinks unavailable on this platform: ${error.code}`);
      return false;
    }
    throw error;
  }
}

async function withStaticServer(rootDirectory, assertion) {
  const server = createStaticServer(rootDirectory);
  await new Promise((resolveListening, rejectListening) => {
    server.once('error', rejectListening);
    server.listen(0, '127.0.0.1', resolveListening);
  });

  try {
    const address = server.address();
    assert(address && typeof address === 'object');
    await assertion(`http://127.0.0.1:${address.port}`);
  } finally {
    await new Promise((resolveClosed, rejectClosed) => {
      server.close((error) => (error ? rejectClosed(error) : resolveClosed()));
    });
  }
}

test('does not serve a file symlink that escapes the static root', async (testContext) => {
  const { outsideDirectory, rootDirectory } = await createFixture();
  const outsideFile = join(outsideDirectory, 'outside.txt');
  await writeFile(outsideFile, 'outside file fixture');
  if (!(await createSymlinkOrSkip(testContext, outsideFile, join(rootDirectory, 'leak.txt'))))
    return;

  await withStaticServer(rootDirectory, async (baseUrl) => {
    const response = await fetch(`${baseUrl}/leak.txt`);
    assert.equal(response.status, 404);
    assert.equal(await response.text(), 'fallback fixture');
  });
});

test('does not serve a directory symlink that escapes the static root', async (testContext) => {
  const { outsideDirectory, rootDirectory } = await createFixture();
  await writeFile(join(outsideDirectory, 'index.html'), 'outside directory fixture');
  if (
    !(await createSymlinkOrSkip(
      testContext,
      outsideDirectory,
      join(rootDirectory, 'leaked-directory'),
    ))
  )
    return;

  await withStaticServer(rootDirectory, async (baseUrl) => {
    const response = await fetch(`${baseUrl}/leaked-directory`);
    assert.equal(response.status, 404);
    assert.equal(await response.text(), 'fallback fixture');
  });
});

test('serves a file symlink whose target remains inside the static root', async (testContext) => {
  const { rootDirectory } = await createFixture();
  const target = join(rootDirectory, 'target.txt');
  await writeFile(target, 'inside file fixture');
  if (!(await createSymlinkOrSkip(testContext, target, join(rootDirectory, 'linked.txt')))) return;

  await withStaticServer(rootDirectory, async (baseUrl) => {
    const response = await fetch(`${baseUrl}/linked.txt`);
    assert.equal(response.status, 200);
    assert.equal(await response.text(), 'inside file fixture');
  });
});

test('does not use a fallback symlink that escapes the static root', async (testContext) => {
  const { outsideDirectory, rootDirectory } = await createFixture();
  const outsideFallback = join(outsideDirectory, 'fallback.html');
  await writeFile(outsideFallback, 'outside fallback fixture');
  await unlink(join(rootDirectory, '404.html'));
  if (!(await createSymlinkOrSkip(testContext, outsideFallback, join(rootDirectory, '404.html'))))
    return;

  await withStaticServer(rootDirectory, async (baseUrl) => {
    const response = await fetch(`${baseUrl}/missing`);
    assert.equal(response.status, 500);
    assert.equal(await response.text(), 'Static preview failed');
  });
});
