#!/usr/bin/env node

const { execSync, spawnSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const checkMode = process.argv.includes('--check');

// Keep in sync with .github/actions/cargo-checks-common-setup/action.yml.
// sqlx-cli 0.9.0 requires rustc >= 1.94, but rust-toolchain.toml pins
// nightly-2025-12-04 (rustc 1.93), so we pin to the last 0.8.x release.
const SQLX_CLI_VERSION = '0.8.6';

function ensureSqlxCli() {
  const check = spawnSync('cargo', ['sqlx', '--version'], { stdio: 'ignore' });
  if (check.status === 0) return;
  console.log(`cargo-sqlx not found; installing sqlx-cli@${SQLX_CLI_VERSION}...`);
  execSync(
    `cargo install sqlx-cli --version ${SQLX_CLI_VERSION} --no-default-features --features sqlite,postgres --locked`,
    { stdio: 'inherit' }
  );
}

ensureSqlxCli();

console.log(checkMode ? 'Checking SQLx prepared queries...' : 'Preparing database for SQLx...');

// Change to backend directory
const backendDir = path.join(__dirname, '..', 'crates/db');
process.chdir(backendDir);

// Create temporary database file
const dbFile = path.join(backendDir, 'prepare_db.sqlite');
fs.writeFileSync(dbFile, '');

try {
  // Get absolute path (cross-platform)
  const dbPath = path.resolve(dbFile);
  const databaseUrl = `sqlite:${dbPath}`;

  console.log(`Using database: ${databaseUrl}`);

  // Run migrations
  console.log('Running migrations...');
  execSync('cargo sqlx migrate run', {
    stdio: 'inherit',
    env: { ...process.env, DATABASE_URL: databaseUrl }
  });

  // Prepare queries
  const sqlxCommand = checkMode ? 'cargo sqlx prepare --check' : 'cargo sqlx prepare';
  console.log(checkMode ? 'Checking prepared queries...' : 'Preparing queries...');
  execSync(sqlxCommand, {
    stdio: 'inherit',
    env: { ...process.env, DATABASE_URL: databaseUrl }
  });

  console.log(checkMode ? 'SQLx check complete!' : 'Database preparation complete!');

} finally {
  // Clean up temporary file
  if (fs.existsSync(dbFile)) {
    fs.unlinkSync(dbFile);
  }
}