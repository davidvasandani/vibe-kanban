#!/usr/bin/env python3
"""Run the actual remote search SQL against an isolated temporary PostgreSQL.
Requires initdb, postgres and psql on PATH. No production credentials or database.
"""
import pathlib
import re
import subprocess
import tempfile
import time

source = (pathlib.Path(__file__).resolve().parents[1] / 'crates/remote/src/routes/global_search.rs').read_text()
query = re.search(r'const SEARCH: &str = r#"(.*?)"#;', source, re.S).group(1)
with tempfile.TemporaryDirectory(prefix='vk-search-pg-') as directory:
    root = pathlib.Path(directory)
    subprocess.run(['initdb', '-D', str(root / 'data'), '-A', 'trust', '--no-locale'], check=True, stdout=subprocess.DEVNULL)
    with (root / 'server.log').open('w') as log:
        server = subprocess.Popen(['postgres', '-D', str(root / 'data'), '-k', directory, '-c', 'listen_addresses=', '-c', 'max_connections=10'], stdout=log, stderr=log)
        try:
            command = ['psql', '-h', directory, '-d', 'postgres', '-X', '-qAt', '-v', 'ON_ERROR_STOP=1']
            for _ in range(100):
                if subprocess.run(command + ['-c', 'SELECT 1'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
                    break
                time.sleep(.1)
            else:
                raise RuntimeError('Temporary PostgreSQL did not start')
            setup = """
            CREATE TABLE organizations(id uuid PRIMARY KEY, name text);
            CREATE TABLE organization_member_metadata(organization_id uuid, user_id uuid);
            CREATE TABLE projects(id uuid PRIMARY KEY, name text, organization_id uuid);
            CREATE TABLE workspaces(id uuid PRIMARY KEY, name text, project_id uuid, local_workspace_id uuid, issue_id uuid, archived boolean, owner_user_id uuid);
            INSERT INTO organizations VALUES
              ('00000000-0000-0000-0000-000000000001', 'Needle org'),
              ('00000000-0000-0000-0000-000000000002', 'Needle secret'),
              ('00000000-0000-0000-0000-000000000003', 'Needle second');
            INSERT INTO organization_member_metadata VALUES
              ('00000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000010'),
              ('00000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000010');
            INSERT INTO projects SELECT id, name || ' project', id FROM organizations;
            INSERT INTO workspaces SELECT id, name || ' workspace', id, id, NULL, true, '00000000-0000-0000-0000-000000000010' FROM organizations;
            INSERT INTO workspaces VALUES ('00000000-0000-0000-0000-000000000099', 'Needle other owner', '00000000-0000-0000-0000-000000000001', NULL, NULL, false, '00000000-0000-0000-0000-000000000011');
            """
            subprocess.run(command, input=setup, text=True, check=True)
            def search(term):
                escaped = term.replace("'", "''")
                return subprocess.run(command, input=f"PREPARE search(uuid,text) AS {query}; EXECUTE search('00000000-0000-0000-0000-000000000010','{escaped}');", text=True, capture_output=True, check=True).stdout.strip().splitlines()
            rows = search('NEEDLE')
            assert len(rows) == 6, rows
            assert not any('secret' in row or 'other owner' in row for row in rows), rows
            assert sum(row.startswith('workspace|') and row.endswith('|t') for row in rows) == 2
            assert search('%_') == []
            assert search("' OR true --") == []
            subprocess.run(command, input="INSERT INTO projects SELECT gen_random_uuid(), 'Needle ' || n, '00000000-0000-0000-0000-000000000001' FROM generate_series(1,30) n;", text=True, check=True)
            assert sum(row.startswith('project|') for row in search('needle')) == 21
            print('PASS: remote search membership, cross-org coverage, ownership, archived results, literal terms and category bounds')
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
