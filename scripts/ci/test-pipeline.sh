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

function jobNeeds(job) {
  const value = directValue(job, 'needs', 4);
  if (!value) return [];
  if (value.startsWith('[') && value.endsWith(']')) {
    return value
      .slice(1, -1)
      .split(',')
      .map((need) => need.trim())
      .filter(Boolean);
  }
  return [value];
}

function hasExactNeeds(job, expected) {
  return JSON.stringify(jobNeeds(job).sort()) === JSON.stringify([...expected].sort());
}

function jobPermission(job, name) {
  return directValue(mappingBlock(job, 'permissions', 4), name, 6);
}

function stepEnvValue(step, name) {
  return directValue(mappingBlock(step, 'env', 8), name, 10);
}

function hasCheckout(jobSteps) {
  return jobSteps.some((step) => directValue(step, 'uses', 8).startsWith('actions/checkout@'));
}

function hasPersistCredentialsFalse(jobSteps) {
  const checkoutSteps = jobSteps.filter((step) =>
    directValue(step, 'uses', 8).startsWith('actions/checkout@'),
  );
  return (
    checkoutSteps.length > 0 &&
    checkoutSteps.every((step) => stepInput(step, 'persist-credentials') === 'false')
  );
}

function executableRunText(jobSteps) {
  return jobSteps.flatMap((step) => runLines(step)).join('\n');
}

function hasTokenExpression(job, jobSteps) {
  const tokenExpression = /\$\{\{\s*(?:github\.token|secrets\.GITHUB_TOKEN)\s*\}\}/;
  const jobEnv = mappingEntries(mappingBlock(job, 'env', 4), 6);
  if (
    [...jobEnv].some(
      ([key, value]) => ['GH_TOKEN', 'GITHUB_TOKEN'].includes(key) || tokenExpression.test(value),
    )
  ) {
    return true;
  }

  return jobSteps.some((step) => {
    const env = mappingEntries(mappingBlock(step, 'env', 8), 10);
    const withInputs = mappingEntries(mappingBlock(step, 'with', 8), 10);
    return (
      [...env].some(
        ([key, value]) => ['GH_TOKEN', 'GITHUB_TOKEN'].includes(key) || tokenExpression.test(value),
      ) ||
      [...withInputs].some(
        ([key, value]) => /token/i.test(key) || tokenExpression.test(value),
      ) ||
      tokenExpression.test(executableRunText([step]))
    );
  });
}

function mappingEntries(text, indent) {
  const entries = new Map();
  const pattern = new RegExp(`^ {${indent}}([A-Za-z0-9_-]+):\\s*(.*)$`, 'gm');
  for (const match of text.matchAll(pattern)) entries.set(match[1], unquote(match[2]));
  return entries;
}

function canonicalCreateFlow(text) {
  const duplicate = text.search(/if \[ "\$release_count" -gt 1 \]; then/);
  const single = text.search(/elif \[ "\$release_count" -eq 1 \]; then/);
  const zero = single < 0 ? -1 : text.indexOf('\nelse', single);
  const end = zero < 0 ? -1 : text.indexOf('\nfi', zero);
  if (!(duplicate >= 0 && duplicate < single && single < zero && zero < end)) return false;

  const duplicateBranch = text.slice(duplicate, single);
  const singleBranch = text.slice(single, zero);
  const zeroBranch = text.slice(zero, end);
  const after = text.slice(end);
  const aggregatesPages =
    /--paginate\s+--slurp/.test(text) ||
    /--paginate[^\n]*[\s\S]*?\|\s*jq\s+-s(?:\s|$)/.test(text);
  return (
    /gh api\b/.test(text) &&
    aggregatesPages &&
    /repos\/\$\{GITHUB_REPOSITORY\}\/releases/.test(text) &&
    /(?:add|flatten)[^\n]*map\(select\(\.tag_name == env\.GITHUB_REF_NAME\)\)/.test(text) &&
    /release_count=.*length/.test(text) &&
    /exit 1/.test(duplicateBranch) &&
    /\.\[0\]\.draft/.test(singleBranch) &&
    /!= true/.test(singleBranch) &&
    /exit 1/.test(singleBranch) &&
    /release_id=.*\.\[0\]\.id/.test(singleBranch) &&
    !/--method POST/.test(singleBranch) &&
    (text.match(/--method POST/g) ?? []).length === 1 &&
    /--method POST/.test(zeroBranch) &&
    /tag_name="\$GITHUB_REF_NAME"/.test(zeroBranch) &&
    /name="Chronacle \$GITHUB_REF_NAME"/.test(zeroBranch) &&
    /draft=true/.test(zeroBranch) &&
    /prerelease=false/.test(zeroBranch) &&
    /release_id=.*\.id/.test(zeroBranch) &&
    /\[\[ "\$release_id" =~ \^\[0-9\]\+\$ \]\]/.test(after) &&
    /echo "release_id=\$release_id" >> "\$GITHUB_OUTPUT"/.test(after)
  );
}

function canonicalAssetUpload(text) {
  const planLoop = text.search(/while IFS= read -r -d '' asset; do/);
  const planDone = text.indexOf('\ndone', planLoop);
  const uploadLoop = text.search(/for index in "\$\{!assets\[@\]\}"; do/);
  const uploadDone = text.indexOf('\ndone', uploadLoop);
  if (!(planLoop >= 0 && planLoop < planDone && planDone < uploadLoop && uploadLoop < uploadDone)) {
    return false;
  }

  const planBody = text.slice(planLoop, planDone);
  const validation = text.slice(planDone, uploadLoop);
  const uploadBody = text.slice(uploadLoop, uploadDone);
  const beforeUpload = text.slice(0, uploadLoop);
  return (
    /find release-assets\/native release-assets\/flatpak/.test(text) &&
    /-print0/.test(text) &&
    ['deb', 'AppImage', 'rpm', 'app.tar.gz', 'dmg', 'msi', 'exe', 'flatpak'].every((suffix) =>
      text.includes(`*.${suffix}`),
    ) &&
    /basename "\$asset"/.test(planBody) &&
    /release-assets\/native\/chronacle-native-macos-arm64\/\*\/Chronacle\.app\.tar\.gz[^\n]*Chronacle_aarch64\.app\.tar\.gz/.test(
      planBody,
    ) &&
    /release-assets\/native\/chronacle-native-macos-x86_64\/\*\/Chronacle\.app\.tar\.gz[^\n]*Chronacle_x64\.app\.tar\.gz/.test(
      planBody,
    ) &&
    /assets\+=\("\$asset"\)/.test(planBody) &&
    /asset_names\+=\("\$asset_name"\)/.test(planBody) &&
    /\$\{#assets\[@\]\}/.test(validation) &&
    /\$\{#asset_names\[@\]\}/.test(validation) &&
    /sort[^\n]*-u|uniq[^\n]*-d/.test(validation) &&
    /exit 1/.test(validation) &&
    !/gh api\b/.test(beforeUpload) &&
    /asset="\$\{assets\[\$index\]\}"/.test(uploadBody) &&
    /asset_name="\$\{asset_names\[\$index\]\}"/.test(uploadBody) &&
    /@uri/.test(uploadBody) &&
    !/--hostname uploads\.github\.com/.test(text) &&
    /--method POST/.test(uploadBody) &&
    /https:\/\/uploads\.github\.com\/repos\/\$\{GITHUB_REPOSITORY\}\/releases\/\$\{RELEASE_ID\}\/assets\?name=\$\{encoded_name\}/.test(
      uploadBody,
    ) &&
    /Content-Type: application\/octet-stream/.test(uploadBody) &&
    /--input "\$asset"/.test(uploadBody)
  );
}

function derivedReleaseAssetName(path) {
  const basename = path.split('/').at(-1);
  if (basename !== 'Chronacle.app.tar.gz') return basename;
  if (path.includes('/chronacle-native-macos-arm64/')) return 'Chronacle_aarch64.app.tar.gz';
  if (path.includes('/chronacle-native-macos-x86_64/')) return 'Chronacle_x64.app.tar.gz';
  return basename;
}

function hasUniqueDerivedReleaseAssetNames(paths) {
  const names = paths.map(derivedReleaseAssetName);
  return names.length > 0 && new Set(names).size === paths.length;
}

function canonicalPublish(text) {
  const releaseEndpoints =
    text.match(
      /repos\/\$\{GITHUB_REPOSITORY\}\/releases\/(?:\$\{RELEASE_ID\}|\$RELEASE_ID)/g,
    ) ?? [];
  const patch = text.indexOf('--method PATCH');
  const beforePatch = patch < 0 ? '' : text.slice(0, patch);
  const patchBody = patch < 0 ? '' : text.slice(patch);
  return (
    releaseEndpoints.length >= 2 &&
    /release_tag=.*\.tag_name/.test(beforePatch) &&
    /release_draft=.*\.draft/.test(beforePatch) &&
    /"\$release_tag" != "\$GITHUB_REF_NAME"/.test(beforePatch) &&
    /"\$release_draft" != true/.test(beforePatch) &&
    /exit 1/.test(beforePatch) &&
    /gh api\b/.test(text) &&
    /repos\/\$\{GITHUB_REPOSITORY\}\/releases\/(?:\$\{RELEASE_ID\}|\$RELEASE_ID)/.test(
      patchBody,
    ) &&
    /draft=false/.test(patchBody) &&
    /prerelease=false/.test(patchBody) &&
    !/gh release edit/.test(text)
  );
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

const on = extractTopLevel('on');
const globalEnv = extractTopLevel('env');
const concurrency = extractTopLevel('concurrency');
const topLevelPermissions = extractTopLevel('permissions');
const topLevelPermissionsDeclaration = /^permissions:[ \t]*(.*)$/m.exec(workflow);
const jobs = extractJobs();
const preCheck = jobs.get('pre-check') ?? '';
const createRelease = jobs.get('create-release') ?? '';
const build = jobs.get('build') ?? '';
const flatpak = jobs.get('flatpak') ?? '';
const uploadReleaseAssets = jobs.get('upload-release-assets') ?? '';
const publish = jobs.get('publish-release') ?? '';
const buildRows = matrixRecords(build);
const flatpakRows = matrixRecords(flatpak);
const preCheckSteps = steps(preCheck);
const createReleaseSteps = steps(createRelease);
const buildSteps = steps(build);
const flatpakSteps = steps(flatpak);
const uploadReleaseAssetSteps = steps(uploadReleaseAssets);
const publishSteps = steps(publish);
const allTauriActionSteps = [...jobs.values()]
  .flatMap((job) => steps(job))
  .filter((step) => directValue(step, 'uses', 8).startsWith('tauri-apps/tauri-action@'));
const tauriReleaseStep =
  buildSteps.find((step) =>
    stepUses(
      step,
      'tauri-apps/tauri-action@84b9d35b5fc46c1e45415bdb6144030364f7ebc5',
    ),
  ) ?? '';
const tauriWith = mappingBlock(tauriReleaseStep, 'with', 8);

requireContract(
  directValue(concurrency, 'group', 2) === '${{ github.workflow }}-${{ github.ref }}' &&
    directValue(concurrency, 'cancel-in-progress', 2) === 'false',
  'release workflow must serialize the exact ref without cancelling in-progress releases',
);
requireContract(
  topLevelPermissionsDeclaration !== null &&
    topLevelPermissionsDeclaration[1] === '' &&
    directValue(topLevelPermissions, 'contents', 2) === 'read',
  'top-level permissions must explicitly set contents read',
);

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
requireContract(
  !canonicalCreateFlow(
    'gh api --method POST repos/${GITHUB_REPOSITORY}/releases -F draft=true\n' +
      'release_id=42\necho "release_id=$release_id" >> "$GITHUB_OUTPUT"',
  ),
  'pipeline parser must reject unconditional release creation',
);
requireContract(
  !canonicalCreateFlow('release_id=123\necho "release_id=$release_id" >> "$GITHUB_OUTPUT"'),
  'pipeline parser must reject arbitrary release IDs',
);
const canonicalCreateFixture = [
  'matching_releases=$(gh api --paginate --slurp "repos/${GITHUB_REPOSITORY}/releases" | jq "add | map(select(.tag_name == env.GITHUB_REF_NAME))")',
  "release_count=$(jq 'length' <<<\"$matching_releases\")",
  'if [ "$release_count" -gt 1 ]; then',
  'exit 1',
  'elif [ "$release_count" -eq 1 ]; then',
  "existing_draft=$(jq -r '.[0].draft' <<<\"$matching_releases\")",
  'if [ "$existing_draft" != true ]; then exit 1; fi',
  "release_id=$(jq -r '.[0].id' <<<\"$matching_releases\")",
  'else',
  'created=$(gh api --method POST "repos/${GITHUB_REPOSITORY}/releases" -f tag_name="$GITHUB_REF_NAME" -f name="Chronacle $GITHUB_REF_NAME" -F draft=true -F prerelease=false)',
  "release_id=$(jq -r '.id' <<<\"$created\")",
  'fi',
  '[[ "$release_id" =~ ^[0-9]+$ ]]',
  'echo "release_id=$release_id" >> "$GITHUB_OUTPUT"',
].join('\n');
requireContract(
  canonicalCreateFlow(canonicalCreateFixture),
  'pipeline parser canonical release fixture must remain valid',
);
const canonicalAssetFixture = [
  'assets=()',
  'asset_names=()',
  "while IFS= read -r -d '' asset; do",
  'asset_name=$(basename "$asset")',
  'case "$asset" in',
  'release-assets/native/chronacle-native-macos-arm64/*/Chronacle.app.tar.gz) asset_name=Chronacle_aarch64.app.tar.gz ;;',
  'release-assets/native/chronacle-native-macos-x86_64/*/Chronacle.app.tar.gz) asset_name=Chronacle_x64.app.tar.gz ;;',
  'esac',
  'assets+=("$asset")',
  'asset_names+=("$asset_name")',
  "done < <(find release-assets/native release-assets/flatpak -type f \\( -name '*.deb' -o -name '*.AppImage' -o -name '*.rpm' -o -name '*.app.tar.gz' -o -name '*.dmg' -o -name '*.msi' -o -name '*.exe' -o -name '*.flatpak' \\) -print0)",
  'if [ "${#assets[@]}" -eq 0 ] || [ "${#assets[@]}" -ne "${#asset_names[@]}" ]; then exit 1; fi',
  'if [ "$(printf \'%s\\n\' "${asset_names[@]}" | sort -u | wc -l)" -ne "${#asset_names[@]}" ]; then exit 1; fi',
  'for index in "${!assets[@]}"; do',
  'asset="${assets[$index]}"',
  'asset_name="${asset_names[$index]}"',
  "encoded_name=$(jq -rn --arg name \"$asset_name\" '$name | @uri')",
  'gh api --method POST "https://uploads.github.com/repos/${GITHUB_REPOSITORY}/releases/${RELEASE_ID}/assets?name=${encoded_name}" -H "Content-Type: application/octet-stream" --input "$asset"',
  'done',
].join('\n');
requireContract(
  canonicalAssetUpload(canonicalAssetFixture),
  'pipeline parser canonical asset fixture must remain valid',
);
requireContract(
  !canonicalAssetUpload(
    canonicalAssetFixture.replace(
      'if [ "$(printf \'%s\\n\' "${asset_names[@]}" | sort -u | wc -l)" -ne "${#asset_names[@]}" ]; then exit 1; fi',
      '',
    ),
  ),
  'pipeline parser must require complete unique asset-name validation before API mutation',
);
requireContract(
  !canonicalAssetUpload(canonicalAssetFixture.replace('https://uploads.github.com/', '').replace('gh api --method POST', 'gh api --hostname uploads.github.com --method POST')),
  'pipeline parser must reject hostname-based upload API calls',
);
requireContract(
  !hasUniqueDerivedReleaseAssetNames([
    'release-assets/native/one/Chronacle.app.tar.gz',
    'release-assets/native/two/Chronacle.app.tar.gz',
  ]),
  'two unmapped macOS archives with the same basename must be rejected',
);
requireContract(
  hasUniqueDerivedReleaseAssetNames([
    'release-assets/native/chronacle-native-macos-arm64/bundle/Chronacle.app.tar.gz',
    'release-assets/native/chronacle-native-macos-x86_64/bundle/Chronacle.app.tar.gz',
  ]),
  'architecture-labelled macOS archive names must remain unique',
);
const canonicalPublishFixture = [
  'release=$(gh api "repos/${GITHUB_REPOSITORY}/releases/${RELEASE_ID}")',
  "release_tag=$(jq -r '.tag_name' <<<\"$release\")",
  "release_draft=$(jq -r '.draft' <<<\"$release\")",
  'if [ "$release_tag" != "$GITHUB_REF_NAME" ] || [ "$release_draft" != true ]; then exit 1; fi',
  'gh api --method PATCH "repos/${GITHUB_REPOSITORY}/releases/${RELEASE_ID}" -F draft=false -F prerelease=false',
].join('\n');
requireContract(
  canonicalPublish(canonicalPublishFixture),
  'pipeline parser canonical publication fixture must remain valid',
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
  allTauriActionSteps.length === 1 && tauriReleaseStep.length > 0,
  'native packaging must use exactly one tauri-action pinned to the reviewed commit',
);
requireContract(
  /^ {10}args:\s*--target \$\{\{ matrix\.target \}\} --features rocksdb\s*$/m.test(tauriWith),
  'native packaging must pass every explicit target to Tauri',
);
requireContract(
  /^ {10}releaseDraft:\s*true\s*$/m.test(tauriWith),
  'native releases must remain drafts during build',
);
requireContract(
  stepInput(tauriReleaseStep, 'tagName') === '' &&
    stepInput(tauriReleaseStep, 'releaseName') === '',
  'tauri-action must remain build-only',
);
requireContract(
  stepEnvValue(tauriReleaseStep, 'GITHUB_TOKEN') === '',
  'tauri-action must not receive GITHUB_TOKEN',
);
const versionConsistencyStep =
  preCheckSteps.find((step) => directValue(step, 'name', 8) === 'Check version consistency') ?? '';
requireContract(
  isReleaseTagGuard(directValue(versionConsistencyStep, 'if', 8)),
  'version consistency must run only for release tags',
);
requireContract(createRelease.length > 0, 'create-release job must exist');
requireContract(
  hasExactNeeds(createRelease, ['pre-check']),
  'create-release must need pre-check',
);
requireContract(
  isReleaseTagGuard(directValue(createRelease, 'if', 4)),
  'create-release must have a job-level release-tag guard',
);
requireContract(
  jobPermission(createRelease, 'contents') === 'write',
  'create-release must have contents write permission',
);
requireContract(!hasCheckout(createReleaseSteps), 'create-release must not check out source code');
const createReleaseOutput = directValue(mappingBlock(createRelease, 'outputs', 4), 'release_id', 6);
requireContract(
  createReleaseOutput === '${{ steps.release.outputs.release_id }}',
  'create-release must expose the exact release ID',
);
const createReleaseStep =
  createReleaseSteps.find((step) => directValue(step, 'id', 8) === 'release') ?? '';
const createReleaseRun = executableRunText([createReleaseStep]);
requireContract(
  stepEnvValue(createReleaseStep, 'GH_TOKEN') === '${{ github.token }}',
  'create-release executable step must bind the job token',
);
requireContract(
  canonicalCreateFlow(createReleaseRun),
  'create-release must canonically select, reuse, or create exactly one draft release ID',
);
requireContract(
  directValue(build, 'if', 4) === '',
  'build job must remain runnable on pull requests',
);
requireContract(
  directValue(flatpak, 'if', 4) === '',
  'flatpak job must remain runnable on pull requests',
);
for (const [name, job, jobSteps] of [
  ['build', build, buildSteps],
  ['flatpak', flatpak, flatpakSteps],
]) {
  requireContract(
    jobPermission(job, 'contents') === 'read',
    `${name} must have contents read permission`,
  );
  requireContract(
    hasPersistCredentialsFalse(jobSteps),
    `${name} checkout must disable persisted credentials`,
  );
  requireContract(
    directValue(mappingBlock(job, 'env', 4), 'GITHUB_TOKEN', 6) === '' &&
      directValue(mappingBlock(job, 'env', 4), 'GH_TOKEN', 6) === '' &&
      !jobSteps.some(
        (step) =>
          stepEnvValue(step, 'GITHUB_TOKEN') !== '' || stepEnvValue(step, 'GH_TOKEN') !== '',
      ),
    `${name} must not receive a GitHub token`,
  );
  requireContract(
    !jobSteps.some(
      (step) =>
        runInvokes(step, /^gh release upload(?:\s|$)/) ||
        /releases\/[^\s]+\/assets/.test(executableRunText([step])),
    ),
    `${name} must not upload release assets directly`,
  );
}
requireContract(
  hasPersistCredentialsFalse(preCheckSteps),
  'pre-check checkout must disable persisted credentials',
);
for (const [name, job, jobSteps] of [
  ['pre-check', preCheck, preCheckSteps],
  ['build', build, buildSteps],
  ['flatpak', flatpak, flatpakSteps],
]) {
  requireContract(
    !hasTokenExpression(job, jobSteps),
    `${name} must not contain a GitHub token expression`,
  );
}
requireContract(
  ![...mappingEntries(globalEnv, 2)].some(
    ([key, value]) =>
      ['GITHUB_TOKEN', 'GH_TOKEN'].includes(key) ||
      /\$\{\{\s*(?:github\.token|secrets\.GITHUB_TOKEN)\s*\}\}/.test(value),
  ),
  'pull-request jobs must not inherit a global GitHub token',
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
  'apps/desktop/src-tauri/tauri.conf.json',
  'Cargo.toml',
  'Cargo.lock',
  'apps/desktop/src-tauri/Cargo.toml',
  'package.json',
  'pnpm-lock.yaml',
  'pnpm-workspace.yaml',
  'apps/desktop/package.json',
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

requireContract(uploadReleaseAssets.length > 0, 'upload-release-assets job must exist');
requireContract(
  hasExactNeeds(uploadReleaseAssets, ['create-release', 'build', 'flatpak']),
  'upload-release-assets must need create-release, build, and flatpak',
);
requireContract(
  isReleaseTagGuard(directValue(uploadReleaseAssets, 'if', 4)),
  'upload-release-assets must have a job-level release-tag guard',
);
requireContract(
  jobPermission(uploadReleaseAssets, 'contents') === 'write',
  'upload-release-assets must have contents write permission',
);
requireContract(
  !hasCheckout(uploadReleaseAssetSteps),
  'upload-release-assets must not check out source code',
);
for (const [pattern, path] of [
  ['chronacle-native-*', 'release-assets/native'],
  ['chronacle-flatpak-*', 'release-assets/flatpak'],
]) {
  requireContract(
    uploadReleaseAssetSteps.some(
      (step) =>
        stepUses(step, 'actions/download-artifact@v4') &&
        stepInput(step, 'pattern') === pattern &&
        stepInput(step, 'path') === path,
    ),
    `upload-release-assets must download ${pattern} workflow artifacts`,
  );
}
const releaseAssetUploadStep =
  uploadReleaseAssetSteps.find(
    (step) =>
      stepEnvValue(step, 'RELEASE_ID') === '${{ needs.create-release.outputs.release_id }}' &&
      stepEnvValue(step, 'GH_TOKEN') === '${{ github.token }}' &&
      canonicalAssetUpload(executableRunText([step])),
  ) ?? '';
requireContract(
  releaseAssetUploadStep.length > 0,
  'upload-release-assets must iterate every package and upload it against the exact release ID',
);
requireContract(
  !uploadReleaseAssetSteps.some((step) => runInvokes(step, /^gh release upload(?:\s|$)/)),
  'upload-release-assets must not address the release by tag',
);

requireContract(publish.length > 0, 'publish-release job must exist');
requireContract(
  hasExactNeeds(publish, ['create-release', 'upload-release-assets', 'build', 'flatpak']),
  'publish-release must need create-release, upload-release-assets, build, and flatpak',
);
const publishReleaseStep =
  publishSteps.find((step) =>
    stepEnvValue(step, 'RELEASE_ID') === '${{ needs.create-release.outputs.release_id }}' &&
    stepEnvValue(step, 'GH_TOKEN') === '${{ github.token }}' &&
    canonicalPublish(executableRunText([step])),
  ) ?? '';
requireContract(
  publishReleaseStep.length > 0,
  'publish-release must validate and publish the exact release ID through REST',
);
const publishJobIf = /^ {4}if:\s*(.+)$/m.exec(publish)?.[1] ?? '';
requireContract(
  isReleaseTagGuard(publishJobIf),
  'publish-release must have a job-level release-tag guard',
);
requireContract(
  jobPermission(publish, 'contents') === 'write',
  'publish-release must have contents write permission',
);
requireContract(!hasCheckout(publishSteps), 'publish-release must not check out source code');

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
const tagOnlyWriteJobs = new Set(['create-release', 'upload-release-assets', 'publish-release']);
for (const [name, job] of jobs) {
  if (jobPermission(job, 'contents') === 'write') {
    requireContract(
      tagOnlyWriteJobs.has(name) && isReleaseTagGuard(directValue(job, 'if', 4)),
      `${name} must not have write permission on pull requests`,
    );
  }
}
for (const [name, job] of jobs) {
  if (name !== 'publish-release') {
    const publishesDraft = steps(job).some((step) => {
      const run = executableRunText([step]);
      return (
        /gh api\b/.test(run) &&
        /--method PATCH/.test(run) &&
        /repos\/\$\{GITHUB_REPOSITORY\}\/releases\//.test(run) &&
        /draft=false/.test(run)
      );
    });
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
