import assert from 'node:assert/strict';
import { chmod, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { afterEach, test } from 'node:test';

import {
  formatterPackages,
  prettierExecutablePath,
  runFormatPrerequisiteCheck,
} from './check-format-prerequisites.mjs';

const fixtureRoots = [];

async function createFixtureRoot() {
  const fixtureRoot = await mkdtemp(
    path.join(os.tmpdir(), 'vibe-format-preflight-')
  );
  fixtureRoots.push(fixtureRoot);
  return fixtureRoot;
}

async function addFormatter(fixtureRoot, packagePath) {
  const executablePath = prettierExecutablePath(fixtureRoot, packagePath);
  await mkdir(path.dirname(executablePath), { recursive: true });
  await writeFile(executablePath, '');
  await chmod(executablePath, 0o755);
}

afterEach(async () => {
  await Promise.all(
    fixtureRoots.splice(0).map((fixtureRoot) =>
      rm(fixtureRoot, { recursive: true, force: true })
    )
  );
});

test('fails early with an actionable install command when formatters are absent', async () => {
  const fixtureRoot = await createFixtureRoot();
  let output = '';

  const exitCode = await runFormatPrerequisiteCheck({
    workspaceRoot: fixtureRoot,
    stderr: {
      write(chunk) {
        output += chunk;
      },
    },
  });

  assert.equal(exitCode, 1);
  assert.match(output, /pnpm install --frozen-lockfile/);
  assert.match(output, /packages\/web-core/);
  assert.match(output, /packages\/local-web/);
  assert.match(output, /packages\/remote-web/);
  assert.doesNotMatch(output, /prettier: command not found/);
});

test('succeeds only when every frontend formatting package has Prettier', async () => {
  const fixtureRoot = await createFixtureRoot();

  for (const packagePath of formatterPackages) {
    await addFormatter(fixtureRoot, packagePath);
  }

  const exitCode = await runFormatPrerequisiteCheck({
    workspaceRoot: fixtureRoot,
  });

  assert.equal(exitCode, 0);
});

test('reports a partially installed workspace instead of skipping a package', async () => {
  const fixtureRoot = await createFixtureRoot();
  let output = '';

  await addFormatter(fixtureRoot, 'packages/web-core');
  await addFormatter(fixtureRoot, 'packages/local-web');

  const exitCode = await runFormatPrerequisiteCheck({
    workspaceRoot: fixtureRoot,
    stderr: {
      write(chunk) {
        output += chunk;
      },
    },
  });

  assert.equal(exitCode, 1);
  assert.match(output, /packages\/remote-web/);
  assert.doesNotMatch(output, /packages\/web-core/);
  assert.doesNotMatch(output, /packages\/local-web/);
});
