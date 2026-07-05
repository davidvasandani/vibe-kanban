import fs from 'fs';
import path from 'path';

// Vibe Kanban runs the binaries built by ./local-build.sh into npx-cli/dist/.
// There is no remote download path — the package ships (or is built with) its
// own binaries under dist/.
export const LOCAL_DIST_DIR = path.join(__dirname, '..', 'dist');

export interface DesktopBundleInfo {
  archivePath: string | null;
  dir: string;
  type: string | null;
}

/**
 * Resolves the local zip for a CLI binary from npx-cli/dist/<platform>/.
 * Throws a helpful error if the build output is missing.
 */
export async function ensureBinary(
  platform: string,
  binaryName: string
): Promise<string> {
  const localZipPath = path.join(LOCAL_DIST_DIR, platform, `${binaryName}.zip`);
  if (fs.existsSync(localZipPath)) {
    return localZipPath;
  }
  throw new Error(
    `Local binary not found: ${localZipPath}\n` +
      `Run ./local-build.sh first to build the binaries.`
  );
}

/**
 * Resolves the local Tauri desktop bundle from npx-cli/dist/tauri/<platform>/.
 * Throws a helpful error if the build output is missing.
 */
export async function ensureDesktopBundle(
  tauriPlatform: string
): Promise<DesktopBundleInfo> {
  const localDir = path.join(LOCAL_DIST_DIR, 'tauri', tauriPlatform);
  if (fs.existsSync(localDir)) {
    const files = fs.readdirSync(localDir);
    const archive = files.find(
      (f) => f.endsWith('.tar.gz') || f.endsWith('-setup.exe')
    );
    return {
      dir: localDir,
      archivePath: archive ? path.join(localDir, archive) : null,
      type: null,
    };
  }
  throw new Error(
    `Local desktop bundle not found: ${localDir}\n` +
      `Run './local-build.sh --desktop' first to build the Tauri app.`
  );
}
