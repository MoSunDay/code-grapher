---
name: feature-versioning
description: Create, refresh, review, and approve git-backed per-repository MR branches of understanding docs.
---

# Feature versioning (MR branch model)

Use the `cg-feature` CLI (subcommands `repo-id`, `mrs`, `tree`, `cat`, `diff`,
`commits`, `draft`, `approve`, `changelog`) or the REST API under
`/api/features/{repo_id}/…` for all lifecycle writes. Read
`docs/feature-versions.md` for the complete contract. **Never store feature
data in the code-grapher source checkout.**

- Identify a repository by `repo_id = sha256(canonical git_url)` (64 lowercase
  hex; compute with `cg-feature repo-id GIT_URL`). Identify an MR solely by
  `name`; its draft lives on branch `mr/<name>`, its files under `docs/<name>/`
  (arbitrary `.md`/`.json` text tree, no fixed schema).
- Lifecycle is pure git: `mr/<name>` tip not in `main` = **open** (draft);
  tip reachable from `main` = **merged** (stable). `approve` runs
  `git merge --no-ff` into `main` with the reviewer as merge author and appends
  a `_changelog/YYYY-MM-DD-<name>-<hash6>.md` entry.
- Refresh a draft with `draft GIT_URL FEATURE_NAME SOURCE_DIR
  --expected-feature-name NAME`. It mirrors SOURCE_DIR into `docs/<NAME>/` on
  branch `mr/<NAME>`, prints `DRAFT <commit>` or `NOOP` (byte-identical), never
  approves, and never touches other MRs' namespaces.
- Publish with `approve GIT_URL FEATURE_NAME --reviewer NAME [--note TEXT]
  [--expected-tip HASH]`. On merge conflict, refresh the draft first. Stable
  state enters exclusively through human review (web 评审页 or this command).
- Inspect with `mrs`, `tree [--mr NAME]`, `cat PATH [--mr NAME]`, `diff NAME`,
  `commits NAME`, `changelog`. Writes serialize via `flock(<repo_id>/.lock)`
  and expect **one writer per partition** (the cg-server instance owning the
  feature root); reads are lock-free git-ref queries.
- Resolve the feature root via `--feature-root`, else `CODE_GRAPHER_DATA_DIR`,
  else `${XDG_DATA_HOME}/code-grapher`, else `${HOME}/.local/share/code-grapher`.
- Run periodic refresh with `scripts/feature-periodic.sh GIT_URL FEATURE_NAME
  SOURCE_DIR [FEATURE_ROOT]`.
