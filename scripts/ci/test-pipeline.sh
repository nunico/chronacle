#!/bin/sh
set -eu

fail() {
  printf 'pipeline contract failed: %s\n' "$1" >&2
  exit 1
}

rg -q '^surrealdb = \{ version = "2", default-features = false, features = \["kv-mem"\] \}$' Cargo.toml \
  || fail 'workspace SurrealDB must be memory-only'
rg -q '^scripts/ci/test-pipeline\.sh$' scripts/ci/backend-quality.sh \
  || fail 'backend quality must enforce the pipeline contract'
rg -q '^[[:space:]]+ripgrep \\$' Dockerfile.ci \
  || fail 'CI Docker image must install ripgrep for pipeline contracts'
! rg -q '^pdfium-render =' apps/desktop/src-tauri/Cargo.toml \
  || fail 'desktop must not activate pdfium-render directly'

metadata_file=$(mktemp "${TMPDIR:-/tmp}/chronacle-metadata.XXXXXX")
trap 'rm -f "$metadata_file"' EXIT HUP INT TERM
cargo metadata --no-deps --format-version 1 >"$metadata_file" \
  || fail 'cargo metadata could not inspect workspace features'

if ! node - "$metadata_file" <<'NODE'
const fs = require('node:fs');

const metadata = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const surrealDependencies = metadata.packages.flatMap((pkg) =>
  pkg.dependencies
    .filter((dependency) => dependency.name === 'surrealdb')
    .map((dependency) => ({ dependency, manifest: pkg.manifest_path })),
);

if (surrealDependencies.length === 0) {
  throw new Error('workspace packages do not inherit SurrealDB');
}

for (const { dependency, manifest } of surrealDependencies) {
  if (dependency.uses_default_features) {
    throw new Error(`SurrealDB defaults are enabled by ${manifest}`);
  }
  if (dependency.features.includes('kv-rocksdb')) {
    throw new Error(`kv-rocksdb is activated directly by ${manifest}`);
  }
}

const desktop = metadata.packages.find((pkg) =>
  pkg.manifest_path.endsWith('/apps/desktop/src-tauri/Cargo.toml'),
);
if (!desktop) {
  throw new Error('desktop package is missing from cargo metadata');
}

const rocksdb = desktop.features.rocksdb;
if (!Array.isArray(rocksdb) || rocksdb.length !== 1 || rocksdb[0] !== 'surrealdb/kv-rocksdb') {
  throw new Error('desktop rocksdb feature must activate only surrealdb/kv-rocksdb');
}

for (const pkg of metadata.packages) {
  for (const [feature, activations] of Object.entries(pkg.features)) {
    if (
      activations.includes('surrealdb/kv-rocksdb') &&
      (pkg.manifest_path !== desktop.manifest_path || feature !== 'rocksdb')
    ) {
      throw new Error(`kv-rocksdb has an unintended feature path in ${pkg.manifest_path}`);
    }
  }
}

const defaults = [...(desktop.features.default || [])];
const reachable = new Set();
while (defaults.length > 0) {
  const feature = defaults.pop();
  if (reachable.has(feature)) continue;
  reachable.add(feature);
  for (const activation of desktop.features[feature] || []) {
    if (!activation.includes('/') && !activation.startsWith('dep:')) defaults.push(activation);
  }
}
if (reachable.has('rocksdb')) {
  throw new Error('desktop rocksdb feature must not be enabled by default');
}
NODE
then
  fail 'Cargo feature boundaries are invalid'
fi

workflow_job_runs_stage() {
  awk -v job="$1" -v script="scripts/ci/$1.sh" '
    $0 == "  " job ":" { in_job = 1; seen = 1; next }
    in_job && $0 ~ /^  [A-Za-z0-9_-]+:/ { in_job = 0 }
    in_job && $0 ~ /^[[:space:]]+run:/ {
      line = $0
      sub(/^[[:space:]]+run:[[:space:]]+/, "", line)
      sub(/[[:space:]]+$/, "", line)
      if (line == script) found = 1
    }
    END { exit !(seen && found) }
  ' .github/workflows/ci.yml
}

docker_stage_runs_stage() {
  awk -v stage="$1" -v script="scripts/ci/$1.sh" '
    $1 == "FROM" && $(NF - 1) == "AS" && $NF == stage {
      in_stage = 1
      seen = 1
      next
    }
    in_stage && $1 == "FROM" { in_stage = 0 }
    in_stage {
      line = $0
      sub(/^[[:space:]]+/, "", line)
      sub(/[[:space:]]+$/, "", line)
      if (line == "RUN " script || line == script) found = 1
    }
    END { exit !(seen && found) }
  ' Dockerfile.ci
}

for stage in backend-quality frontend-quality acceptance; do
  test -x "scripts/ci/$stage.sh" || fail "$stage entrypoint is missing"
  workflow_job_runs_stage "$stage" \
    || fail "$stage workflow does not use its shared entrypoint"
  docker_stage_runs_stage "$stage" \
    || fail "$stage Docker target does not use its shared entrypoint"
done

rg -q 'run: pnpm -C apps/desktop exec tauri build --no-bundle --features rocksdb' \
  .github/workflows/ci.yml \
  || fail 'main production build must build the desktop app with rocksdb'
rg -q 'run: pnpm exec tauri build --no-bundle --features rocksdb' \
  .github/workflows/e2e-ui.yml \
  || fail 'UI E2E workflow build must enable rocksdb'
rg -q '^RUN pnpm -C apps/desktop exec tauri build --no-bundle --features rocksdb$' \
  apps/desktop/tests/e2e/ui/Dockerfile \
  || fail 'UI E2E container build must enable rocksdb'

rg -q '^[[:space:]]*run: cargo test -p Chronacle --features rocksdb[[:space:]]*$' \
  .github/workflows/release.yml \
  || fail 'release validation must exercise the RocksDB-enabled desktop suite'

node <<'NODE'
const fs = require('node:fs');

const workflow = fs.readFileSync('.github/workflows/release.yml', 'utf8');
const failures = [];

function requireContract(condition, message) {
  if (!condition) failures.push(message);
}

function extractTopLevel(name) {
  const match = new RegExp(`^${name}:\\s*$`, 'm').exec(workflow);
  if (!match) return '';
  const tail = workflow.slice(match.index + match[0].length);
  const end = /^\S[^:]*:\s*$/m.exec(tail);
  return end ? tail.slice(0, end.index) : tail;
}

function extractJobs() {
  const jobsStart = /^jobs:\s*$/m.exec(workflow);
  if (!jobsStart) return new Map();

  const jobsText = workflow.slice(jobsStart.index + jobsStart[0].length);
  const starts = [...jobsText.matchAll(/^ {2}([A-Za-z0-9_-]+):\s*$/gm)];
  const jobs = new Map();
  for (let index = 0; index < starts.length; index += 1) {
    const start = starts[index];
    const end = starts[index + 1]?.index ?? jobsText.length;
    jobs.set(start[1], jobsText.slice(start.index, end));
  }
  return jobs;
}

function mappingBlock(text, name, indent) {
  const prefix = ' '.repeat(indent);
  const start = new RegExp(`^${prefix}${name}:\\s*$`, 'm').exec(text);
  if (!start) return '';

  const tail = text.slice(start.index + start[0].length);
  const sibling = new RegExp(`^${prefix}[A-Za-z0-9_-]+:\\s*`, 'm').exec(tail);
  return sibling ? tail.slice(0, sibling.index) : tail;
}

function matrixRecords(job) {
  if (!job) return [];
  const starts = [...job.matchAll(/^ {10}- ([A-Za-z0-9_-]+):\s*(.*)$/gm)];
  return starts.map((start, index) => {
    const end = starts[index + 1]?.index ?? job.length;
    const recordText = job.slice(start.index, end);
    const record = { [start[1]]: unquote(start[2]) };
    for (const line of recordText.matchAll(/^ {12}([A-Za-z0-9_-]+):\s*(.*)$/gm)) {
      record[line[1]] = unquote(line[2]);
    }
    return record;
  });
}

function steps(job) {
  if (!job) return [];
  const starts = [...job.matchAll(/^ {6}- /gm)];
  return starts.map((start, index) => {
    const end = starts[index + 1]?.index ?? job.length;
    return job.slice(start.index, end);
  });
}

function stepInput(step, name) {
  const withBlock = mappingBlock(step, 'with', 8);
  return unquote(new RegExp(`^ {10}${name}:\\s*(.+)$`, 'm').exec(withBlock)?.[1] ?? '');
}

function stepUses(step, action) {
  return new RegExp(`^ {8}uses:\\s*${action}\\s*$`, 'm').test(step);
}

function artifactUploadStep(jobSteps, expectedName, pathPattern) {
  return (
    jobSteps.find(
      (step) =>
        stepUses(step, 'actions/upload-artifact@v4') &&
        stepInput(step, 'name') === expectedName &&
        pathPattern.test(stepInput(step, 'path')),
    ) ?? ''
  );
}

function runLines(step) {
  const run = /^ {8}run:\s*(.*)$/m.exec(step);
  if (!run) return [];
  if (run[1] !== '|') return [unquote(run[1])];

  const tail = step.slice(run.index + run[0].length);
  const sibling = /^ {8}[A-Za-z0-9_-]+:\s*/m.exec(tail);
  const body = sibling ? tail.slice(0, sibling.index) : tail;
  return body
    .split('\n')
    .filter((line) => /^ {10}/.test(line))
    .map((line) => line.slice(10).trim())
    .filter((line) => line !== '' && !line.startsWith('#'));
}

function runInvokes(step, commandPattern) {
  return runLines(step).some((line) => commandPattern.test(line));
}

function unquote(value) {
  return value.trim().replace(/^(["'])(.*)\1$/, '$2');
}

function isReleaseTagGuard(value) {
  return /^(?:\$\{\{\s*)?startsWith\(github\.ref, 'refs\/tags\/v'\)(?:\s*\}\})?$/.test(
    value.trim(),
  );
}

function prSafeReleaseInput(withBlock, name) {
  const input = new RegExp(`^ {10}${name}:\\s*(.+)$`, 'm').exec(withBlock)?.[1] ?? '';
  return (
    /^\$\{\{\s*startsWith\(github\.ref, 'refs\/tags\/v'\)\s*&&\s*.+\s*\|\|\s*''\s*\}\}$/.test(
      input.trim(),
    ) && input.includes('github.ref_name')
  );
}

const on = extractTopLevel('on');
const jobs = extractJobs();
const build = jobs.get('build') ?? '';
const flatpak = jobs.get('flatpak') ?? '';
const publish = jobs.get('publish-release') ?? '';
const buildRows = matrixRecords(build);
const flatpakRows = matrixRecords(flatpak);
const buildSteps = steps(build);
const flatpakSteps = steps(flatpak);
const tauriReleaseStep =
  buildSteps.find((step) => stepUses(step, 'tauri-apps/tauri-action@v0')) ?? '';
const tauriWith = mappingBlock(tauriReleaseStep, 'with', 8);

const splitStepFixture = [
  '  fixture:',
  '    steps:',
  '      - uses: actions/upload-artifact@v4',
  '        with:',
  '          name: wrong-name',
  '          path: target/${{ matrix.target }}/release/bundle/deb/*.deb',
  '      - uses: example/other-action@v1',
  '        with:',
  '          name: chronacle-deb-${{ matrix.flatpak_arch }}',
  '          path: target/${{ matrix.target }}/release/bundle/deb/*.deb',
  '      - run: |',
  '          # scripts/release-flatpak.sh artifacts/*.deb 1.2.3 flatpak-out',
  '          echo no-build',
].join('\n');
const splitFixtureSteps = steps(splitStepFixture);
requireContract(
  artifactUploadStep(
    splitFixtureSteps,
    'chronacle-deb-${{ matrix.flatpak_arch }}',
    /^target\/\$\{\{ matrix\.target \}\}\/release\/bundle\/deb\/\*\.deb$/,
  ).length === 0,
  'pipeline parser must not combine artifact fields from separate steps',
);
requireContract(
  !splitFixtureSteps.some((step) =>
    runInvokes(step, /^(?:exec\s+)?scripts\/release-flatpak\.sh(?:\s|$)/),
  ),
  'pipeline parser must not treat comments as Flatpak build commands',
);

const nativePairs = [
  ['ubuntu-24.04', 'x86_64-unknown-linux-gnu'],
  ['ubuntu-24.04-arm', 'aarch64-unknown-linux-gnu'],
  ['macos-26', 'aarch64-apple-darwin'],
  ['macos-15-intel', 'x86_64-apple-darwin'],
  ['windows-2025', 'x86_64-pc-windows-msvc'],
];

requireContract(buildRows.length === nativePairs.length, 'build must define exactly five native matrix rows');
for (const [os, target] of nativePairs) {
  requireContract(
    buildRows.some((row) => row.os === os && row.target === target),
    `build matrix must pair ${os} with ${target}`,
  );
}

requireContract(
  buildRows.some(
    (row) =>
      row.os === 'ubuntu-24.04' && row.deb_arch === 'amd64' && row.flatpak_arch === 'x86_64',
  ),
  'Linux x86_64 must expose amd64 and x86_64 artifact labels',
);
requireContract(
  buildRows.some(
    (row) =>
      row.os === 'ubuntu-24.04-arm' && row.deb_arch === 'arm64' && row.flatpak_arch === 'aarch64',
  ),
  'Linux aarch64 must expose arm64 and aarch64 artifact labels',
);
requireContract(
  /^ {10}args:\s*--target \$\{\{ matrix\.target \}\} --features rocksdb\s*$/m.test(tauriWith),
  'native packaging must pass every explicit target to Tauri',
);
requireContract(
  /^ {10}releaseDraft:\s*true\s*$/m.test(tauriWith),
  'native releases must remain drafts during build',
);
const debUploadStep = artifactUploadStep(
  buildSteps,
  'chronacle-deb-${{ matrix.flatpak_arch }}',
  /^target\/\$\{\{ matrix\.target \}\}\/release\/bundle\/deb\/\*\.deb$/,
);
requireContract(
  debUploadStep.length > 0,
  'Linux Debian artifacts must be uploaded with architecture labels',
);
const nativeUploadStep = artifactUploadStep(
  buildSteps,
  'chronacle-native-${{ matrix.name }}',
  /^target\/\$\{\{ matrix\.target \}\}\/release\/bundle\/?$/,
);
requireContract(
  nativeUploadStep.length > 0,
  'every native bundle must be retained as an inspectable workflow artifact',
);

const pullRequest = mappingBlock(on, 'pull_request', 2);
const pullRequestPaths = mappingBlock(pullRequest, 'paths', 4);
const pullRequestPathEntries = [...pullRequestPaths.matchAll(/^ {6}-\s*(.+)$/gm)].map((entry) =>
  unquote(entry[1]),
);
requireContract(pullRequest.length > 0, 'release workflow must run on pull requests');
requireContract(
  pullRequestPaths.length > 0,
  'release pull request trigger must be path-filtered',
);
for (const path of [
  '.github/workflows/release.yml',
  'packaging/flatpak/**',
  'scripts/release-flatpak.sh',
  'scripts/ci/test-release-flatpak.sh',
  'scripts/ci/test-pipeline.sh',
  'apps/desktop/src-tauri/build.rs',
  'apps/desktop/src-tauri/src/runtime_downloads.rs',
  'apps/desktop/src-tauri/src/runtime_downloads_tests.rs',
  'README.md',
  'docs/user-guide.md',
  'docs/architecture.md',
]) {
  requireContract(
    pullRequestPathEntries.includes(path),
    `release pull request paths must include ${path}`,
  );
}

requireContract(flatpak.length > 0, 'flatpak job must exist');
requireContract(/^ {4}needs:\s*build\s*$/m.test(flatpak), 'flatpak job must need build');
for (const [os, arch] of [
  ['ubuntu-24.04', 'x86_64'],
  ['ubuntu-24.04-arm', 'aarch64'],
]) {
  requireContract(
    flatpakRows.some((row) => row.os === os && row.arch === arch),
    `flatpak matrix must pair ${os} with ${arch}`,
  );
}
requireContract(
  flatpakSteps.some((step) =>
    runInvokes(step, /^(?:exec\s+)?scripts\/release-flatpak\.sh(?:\s|$)/),
  ),
  'flatpak job must invoke the shared release script',
);
const flatpakWorkflowUploadStep = artifactUploadStep(
  flatpakSteps,
  'chronacle-flatpak-${{ matrix.arch }}',
  /^flatpak-out\/\*\.flatpak$/,
);
requireContract(
  flatpakWorkflowUploadStep.length > 0,
  'flatpak bundles must be retained as workflow artifacts',
);

const flatpakReleaseStep =
  flatpakSteps.find((step) =>
    runInvokes(step, /^gh release upload\b[^#]*\*\.flatpak(?:\s|$)/),
  ) ?? '';
const flatpakReleaseIf = /^ {8}if:\s*(.+)$/m.exec(flatpakReleaseStep)?.[1] ?? '';
requireContract(
  flatpakReleaseStep.length > 0,
  'flatpak bundles must be uploaded to the draft release',
);
requireContract(
  isReleaseTagGuard(flatpakReleaseIf),
  'flatpak release upload must be guarded by a release tag',
);

requireContract(publish.length > 0, 'publish-release job must exist');
requireContract(
  /^ {4}needs:\s*\[build, flatpak\]\s*$/m.test(publish),
  'publish-release must need build and flatpak',
);
requireContract(
  /gh release edit[^\n]*--draft=false/.test(publish),
  'publish-release must remove draft status with gh release edit',
);
const publishJobIf = /^ {4}if:\s*(.+)$/m.exec(publish)?.[1] ?? '';
requireContract(
  isReleaseTagGuard(publishJobIf),
  'publish-release must have a job-level release-tag guard',
);

requireContract(
  prSafeReleaseInput(tauriWith, 'tagName'),
  'native tagName must select the tag only for release tags and be empty on pull requests',
);
requireContract(
  prSafeReleaseInput(tauriWith, 'releaseName'),
  'native releaseName must select a name only for release tags and be empty on pull requests',
);
requireContract(
  !/^ {8}if:\s*.*startsWith\(github\.ref, 'refs\/tags\/v'\)/m.test(tauriReleaseStep),
  'native packaging must remain runnable on pull requests',
);
requireContract(
  !/releaseDraft:\s*false/.test(build),
  'build must never publish a release directly',
);
requireContract(
  !/releaseDraft:\s*false/.test(flatpak),
  'flatpak must never publish a release directly',
);
for (const [name, job] of jobs) {
  if (name !== 'publish-release') {
    requireContract(!job.includes('--draft=false'), `only publish-release may remove draft status, found in ${name}`);
  }
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`pipeline contract failed: ${failure}`);
  process.exit(1);
}
NODE
