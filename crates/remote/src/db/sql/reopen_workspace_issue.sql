UPDATE issues AS issue
SET status_id = target.id, updated_at = NOW()
FROM project_statuses AS current, project_statuses AS target
WHERE issue.id = $1
  AND issue.project_id = $2
  AND current.id = issue.status_id
  AND current.project_id = issue.project_id
  AND LOWER(current.name) = LOWER('Done')
  AND target.project_id = issue.project_id
  AND LOWER(target.name) = LOWER('In progress')
