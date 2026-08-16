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
  const boundary = new RegExp(`^ {0,${indent}}[A-Za-z0-9_-]+:\\s*`, 'm').exec(tail);
  return boundary ? tail.slice(0, boundary.index) : tail;
}

function matrixRecords(job) {
  const strategy = mappingBlock(job, 'strategy', 4);
  const matrix = mappingBlock(strategy, 'matrix', 6);
  const include = mappingBlock(matrix, 'include', 8);
  if (!include) return [];

  const starts = [...include.matchAll(/^ {10}- ([A-Za-z0-9_-]+):\s*(.*)$/gm)];
  return starts.map((start, index) => {
    const end = starts[index + 1]?.index ?? include.length;
    const recordText = include.slice(start.index, end);
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

function directValue(text, name, indent) {
  const value = new RegExp(`^ {${indent}}${name}:\\s*(.+)$`, 'm').exec(text)?.[1] ?? '';
  return unquote(value);
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

function isLinuxOnlyGuard(value) {
  return /^(?:\$\{\{\s*)?runner\.os == 'Linux'(?:\s*\}\})?$/.test(value.trim());
}

function sameRecord(actual, expected) {
  const actualKeys = Object.keys(actual).sort();
  const expectedKeys = Object.keys(expected).sort();
  return (
    JSON.stringify(actualKeys) === JSON.stringify(expectedKeys) &&
    expectedKeys.every((key) => actual[key] === expected[key])
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
const preCheck = jobs.get('pre-check') ?? '';
const build = jobs.get('build') ?? '';
const flatpak = jobs.get('flatpak') ?? '';
const publish = jobs.get('publish-release') ?? '';
const buildRows = matrixRecords(build);
const flatpakRows = matrixRecords(flatpak);
const preCheckSteps = steps(preCheck);
const buildSteps = steps(build);
const flatpakSteps = steps(flatpak);
const publishSteps = steps(publish);
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
  '          # releaseDraft: false',
  '          # gh release edit example --draft=false --prerelease=false',
  '          echo --draft=false',
  '          echo no-build',
].join('\n');
const splitFixtureSteps = steps(splitStepFixture);
requireContract(
  matrixRecords(splitStepFixture).length === 0,
  'pipeline parser must read matrix rows only from strategy.matrix.include',
);
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
requireContract(
  !splitFixtureSteps.some((step) =>
    runInvokes(step, /^gh release edit\b[^#]*--draft=false(?:\s|$)/),
  ),
  'pipeline parser must not treat comments or diagnostics as draft publication',
);

const nativeRows = [
  {
    name: 'linux-x86_64',
    os: 'ubuntu-24.04',
    target: 'x86_64-unknown-linux-gnu',
    deb_arch: 'amd64',
    flatpak_arch: 'x86_64',
  },
  {
    name: 'linux-aarch64',
    os: 'ubuntu-24.04-arm',
    target: 'aarch64-unknown-linux-gnu',
    deb_arch: 'arm64',
    flatpak_arch: 'aarch64',
  },
  { name: 'macos-arm64', os: 'macos-26', target: 'aarch64-apple-darwin' },
  { name: 'macos-x86_64', os: 'macos-15-intel', target: 'x86_64-apple-darwin' },
  { name: 'windows-x86_64', os: 'windows-2025', target: 'x86_64-pc-windows-msvc' },
];

requireContract(buildRows.length === nativeRows.length, 'build must define exactly five native matrix rows');
for (const expected of nativeRows) {
  requireContract(
    buildRows.some((row) => sameRecord(row, expected)),
    `build matrix must define the complete ${expected.name} row`,
  );
}
requireContract(
  /^ {10}args:\s*--target \$\{\{ matrix\.target \}\} --features rocksdb\s*$/m.test(tauriWith),
  'native packaging must pass every explicit target to Tauri',
);
requireContract(
  /^ {10}releaseDraft:\s*true\s*$/m.test(tauriWith),
  'native releases must remain drafts during build',
);
const versionConsistencyStep =
  preCheckSteps.find((step) => directValue(step, 'name', 8) === 'Check version consistency') ?? '';
requireContract(
  isReleaseTagGuard(directValue(versionConsistencyStep, 'if', 8)),
  'version consistency must run only for release tags',
);
requireContract(
  directValue(build, 'if', 4) === '',
  'build job must remain runnable on pull requests',
);
requireContract(
  directValue(flatpak, 'if', 4) === '',
  'flatpak job must remain runnable on pull requests',
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
requireContract(
  isLinuxOnlyGuard(directValue(debUploadStep, 'if', 8)),
  'Debian artifact upload must run only for Linux matrix rows',
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
const expectedFlatpakRows = [
  { os: 'ubuntu-24.04', arch: 'x86_64' },
  { os: 'ubuntu-24.04-arm', arch: 'aarch64' },
];
requireContract(
  flatpakRows.length === expectedFlatpakRows.length,
  'flatpak matrix must define exactly two rows',
);
for (const expected of expectedFlatpakRows) {
  requireContract(
    flatpakRows.some((row) => sameRecord(row, expected)),
    `flatpak matrix must pair ${expected.os} with ${expected.arch}`,
  );
}
const debDownloadStep =
  flatpakSteps.find(
    (step) =>
      stepUses(step, 'actions/download-artifact@v4') &&
      stepInput(step, 'name') === 'chronacle-deb-${{ matrix.arch }}' &&
      stepInput(step, 'path') === 'artifacts',
  ) ?? '';
requireContract(
  debDownloadStep.length > 0,
  'flatpak must download the matching Debian artifact into artifacts',
);
const flatpakBuildStep =
  flatpakSteps.find((step) =>
    runInvokes(
      step,
      /^scripts\/release-flatpak\.sh artifacts\/\*\.deb "\$\{GITHUB_REF_NAME#v\}" flatpak-out$/,
    ),
  ) ?? '';
requireContract(
  flatpakBuildStep.length > 0,
  'flatpak job must invoke the shared release script',
);
requireContract(
  directValue(flatpakBuildStep, 'if', 8) === '',
  'flatpak builder step must remain runnable on pull requests',
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
const publishReleaseStep =
  publishSteps.find((step) =>
    runInvokes(
      step,
      /^gh release edit\b[^#]*--draft=false(?:\s|$)[^#]*--prerelease=false(?:\s|$)/,
    ),
  ) ?? '';
requireContract(
  publishReleaseStep.length > 0,
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
  directValue(tauriReleaseStep, 'if', 8) === '',
  'native packaging must remain runnable on pull requests',
);
for (const [name, jobSteps] of [
  ['build', buildSteps],
  ['flatpak', flatpakSteps],
]) {
  for (const step of jobSteps.filter((candidate) =>
    directValue(candidate, 'uses', 8).startsWith('tauri-apps/tauri-action@'),
  )) {
    requireContract(
      stepInput(step, 'releaseDraft') !== 'false',
      `${name} must never publish a release directly`,
    );
  }
}
for (const [name, job] of jobs) {
  if (name !== 'publish-release') {
    const publishesDraft = steps(job).some((step) =>
      runInvokes(step, /^gh release edit\b[^#]*--draft=false(?:\s|$)/),
    );
    requireContract(
      !publishesDraft,
      `only publish-release may remove draft status, found in ${name}`,
    );
  }
}

if (failures.length > 0) {
  for (const failure of failures) console.error(`pipeline contract failed: ${failure}`);
  process.exit(1);
}
NODE
