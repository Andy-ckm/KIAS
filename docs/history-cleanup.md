# Sensitive History Cleanup

Removing a file in a new commit does not remove it from earlier Git revisions. This repository previously tracked local development state, an API-key queue containing placeholders, bundled reference material, and an internal scan report that described local credential handling. Those paths must be removed from reachable history before treating the repository as sanitized.

## First response: rotate credentials

History rewriting is not credential revocation. Before modifying Git history:

1. revoke or rotate every API key, bot token, webhook secret, JWT key, TLS key, database credential, and external-provider credential ever used on a machine or CI environment associated with this project;
2. review provider audit records for unexpected source addresses, usage, costs, and timestamps;
3. remove old repository/environment secrets and create new least-privilege values;
4. record the incident and rotation date privately without copying secret values.

## Coordinated rewrite

A rewrite changes commit IDs and disrupts forks, clones, branches, tags, pull requests, and signed commits. Coordinate a maintenance window:

- pause pushes;
- merge or record open work;
- create an encrypted offline backup;
- notify collaborators that old clones must never push again;
- run the rewrite from a fresh mirror clone;
- review all refs and scan the rewritten object database;
- force-push once, then require fresh clones.

The guarded helper prints a dry-run plan by default:

```bash
bash scripts/rewrite_sensitive_history.sh
```

Only from a reviewed fresh mirror clone:

```bash
bash scripts/rewrite_sensitive_history.sh --execute
```

## Paths included in the rewrite

- local development state and logs;
- goal, orchestration, research, trace, and task-queue state;
- the tracked API-key queue path;
- the internal security-scan report that discussed local real credentials;
- bundled papers and local reference mirrors.

The exact list is maintained in `scripts/rewrite_sensitive_history.sh` so the procedure is reviewable.

## Validation

After rewriting, scan every reachable object rather than only the working tree:

```bash
git rev-list --objects --all
trufflehog git file://"$(pwd)" --only-verified --fail
```

Also search for private organization/customer aliases and domains using a private denylist. Do not print the denylist or matching source text in shared logs.

## Copies outside Git

A rewritten repository does not remove data from:

- forks and old local clones;
- CI logs and workflow artifacts;
- release archives and package registries;
- search-engine or CDN caches;
- issue, pull-request, discussion, or chat attachments;
- screenshots, backups, mirrors, or copied patches.

Review and remove those copies separately. Credentials must remain revoked even if every known copy is deleted.

## GitHub settings after the rewrite

After the cleaned history is pushed:

- protect the default branch with required pull requests and checks;
- disallow force pushes and deletion on the default branch;
- require review for security-sensitive paths through CODEOWNERS;
- enable secret scanning and push protection where available;
- enable private vulnerability reporting;
- restrict workflow token permissions to read-only by default;
- require approval for workflows from external contributors;
- invalidate old releases and regenerate artifacts from cleaned commits.

## Completion criteria

The cleanup is complete only when:

- affected credentials are rotated;
- all intended refs are rewritten and force-pushed;
- verified-secret and private-identifier scans pass across all objects;
- releases/artifacts/caches are reviewed;
- collaborators have discarded old clones;
- protected-branch and security settings are active;
- the cleanup has been independently reviewed by someone who did not perform it.
