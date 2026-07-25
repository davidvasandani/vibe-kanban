import { access } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const formatterPackages = [
  'packages/web-core',
  'packages/local-web',
  'packages/remote-web',
];

export function prettierExecutablePath(workspaceRoot, packagePath) {
  const executable = process.platform === 'win32' ? 'prettier.cmd' : 'prettier';
  return path.join(workspaceRoot, packagePath, 'node_modules', '.bin', executable);
}

export async function findMissingFormatters(workspaceRoot) {
  const missing = [];

  for (const packagePath of formatterPackages) {
    try {
      await access(prettierExecutablePath(workspaceRoot, packagePath));
    } catch {
      missing.push(packagePath);
    }
  }

  return missing;
}

export async function runFormatPrerequisiteCheck({
  workspaceRoot = path.resolve(
    path.dirname(fileURLToPath(import.meta.url)),
    '..'
  ),
  stderr = process.stderr,
} = {}) {
  const missing = await findMissingFormatters(workspaceRoot);

  if (missing.length === 0) {
    return 0;
  }

  stderr.write(
    [
      'Frontend formatting dependencies are not installed for this worktree.',
      `Missing Prettier executable for: ${missing.join(', ')}`,
      'Run "pnpm install --frozen-lockfile" from the repository root, then retry "pnpm run format".',
      '',
    ].join('\n')
  );
  return 1;
}

const isMain =
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;

if (isMain) {
  process.exitCode = await runFormatPrerequisiteCheck();
}
