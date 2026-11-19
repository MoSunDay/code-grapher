---
name: feature-versioning
description: Create, refresh, review, and approve git-backed per-repository feature versions.
---

# Feature versioning

Use the `cg-feature` CLI (subcommands `repo-id`, `list`, `status`, `diff`, `draft`,
`approve`) or the REST API under `/api/features/{repo_id}` for all lifecycle writes.
Read `docs/feature-versions.md` for the complete contract. **Never store feature data in
the code-grapher source checkout.**

- Identify a repository by `repo_id = sha256(canonical git_url)` (64 lowercase hex;
  compute with `cg-feature repo-id GIT_URL`). Identify a feature solely by `name`, which
  is also its directory name; the legacy `id` field is rejected as an unknown field.
- Lifecycle state lives only in git: an uncommitted `feature.json` change means draft,
  an approve commit means stable. There is no `index.json`, no `draft/vNNN` or
  `verified/vNNN` directories, and no database rows or version metadata in JSON.
- Inspect with `list GIT_URL`, `status GIT_URL`, `diff GIT_URL [FEATURE_NAME]`; state is
  derived purely from `git diff HEAD`, never from stored fields or pointer files.
- Refresh a draft with `draft GIT_URL SOURCE_JSON --expected-feature-name NAME`. It
  prints `DRAFT <rel_path>` or `NOOP` (byte-identical content) and never commits; treat
  `NOOP` as "nothing to do", not an error.
- Publish with `approve GIT_URL FEATURE_NAME --reviewer NAME [--note TEXT]
  [--expected-hash HASH]`. `--expected-hash` is the sha256 of the current feature.json
  bytes; approve commits only that feature.json plus its generated `_changelog/` entry
  and never sweeps other dirty files.
- Resolve the feature root via `--feature-root`, else `CODE_GRAPHER_DATA_DIR`, else
  `${XDG_DATA_HOME}/code-grapher`, else `${HOME}/.local/share/code-grapher`.
- Run periodic refresh with `scripts/feature-periodic.sh GIT_URL FEATURE_NAME SOURCE_JSON
  [FEATURE_ROOT]`. It only refreshes the draft and stops on `NOOP`; stable state enters
  exclusively through human review (web 审核页 or `cg-feature approve`).
