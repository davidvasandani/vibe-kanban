// Time one POST /api/workspaces/summaries per archive scope.
// Usage: node scripts/time-workspace-summaries.mjs [baseUrl]
// Default base URL is the coordinator's direct address (behind Cloudflare
// Access publicly). See docs/analysis/coordinator-nfs-io-pressure.md.
const base = process.argv[2] ?? 'http://172.16.100.102:3334';
for (const archived of [false, true]) {
  const started = Date.now();
  const response = await fetch(`${base}/api/workspaces/summaries`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ archived }),
  });
  const body = await response.json();
  const summaries = body.data?.summaries ?? [];
  console.log(
    JSON.stringify({
      archived,
      status: response.status,
      ms: Date.now() - started,
      rows: summaries.length,
      with_stats: summaries.filter((s) => s.files_changed != null).length,
    })
  );
}
