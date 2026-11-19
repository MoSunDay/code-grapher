# Git-backed feature review (git_url identity)

Feature lifecycle state comes **only** from git. JSON never stores a version,
lifecycle state, or pointer; there are no `draft/`, `verified/`, `vNNN/`
directories and no `index.json`. The database is not involved: no rows, no
autoincrement IDs, no name mapping.

## Identity

- Logical repository identity: the **canonical git_url**.
- Physical directory name: `repo_id = sha256(canonical_git_url)` (64 lowercase
  hex characters). URLs are never stored as paths, so credentials, illegal
  characters, traversal, and length are non-issues.
- Feature identity: `name` — the only ID and the directory name. The legacy
  `id` field is rejected as an unknown field.

### git_url canonicalization

`https://github.com/owner/repo.git`, `ssh://git@github.com/owner/repo.git`, and
`git@github.com:owner/repo.git` all canonicalize to `github.com/owner/repo`:

1. drop the scheme; 2. drop userinfo (`user:pass@` never enters the identity);
3. lowercase the host; 4. trim trailing `/`; 5. drop a trailing `.git`;
6. ignore query and fragment; 7. keep path casing (`Repo` ≠ `repo`);
8. unparseable URLs disable feature storage (`feature_repo_id: null`).

## Layout

```text
<feature-root>/
  3f9c1e5a…/                     # repo_id = sha256(canonical git_url)
    AST 解析与领域模型/
      feature.json
    请求链路推断/
      feature.json
    _changelog/
      2026-11-18-AST解析与领域模型-a1b2c3.md
      2026-11-18-请求链路推断-d4e5f6.md
```

### feature.json

```json
{
  "name": "AST 解析与领域模型",
  "function_entry": "…",
  "request_chain": [],
  "external_dependencies": [],
  "function_summary": "…",
  "children": []
}
```

### feature_name rules

Allowed: unicode text (Chinese included), spaces, letters, digits, `-`, `_`,
`.`. Rejected: empty, `.`/`..`, `/` or `\`, control characters (incl. NUL),
leading/trailing whitespace, names starting with `_` (reserved namespace, e.g.
`_changelog`) or `.`, names over 128 bytes. Names are case-sensitive: `AST`
and `ast` are two features. Same name + changed content = a new draft for the
same feature; there are no duplicate version directories.

## Lifecycle (git semantics)

```text
git diff HEAD non-empty -> draft
git diff HEAD empty     -> stable
approve -> commit        -> merged into stable
```

`approve` commits **only** `<repo_id>/<name>/feature.json` and the generated
`<repo_id>/_changelog/YYYY-MM-DD-<name>-<hash6>.md`. Other dirty files are
never swept into the commit. Commits, features, and changelog entries are
never pruned or capped.

All git invocations run with `GIT_CONFIG_GLOBAL=/dev/null` and
`GIT_CONFIG_NOSYSTEM=1`, so global ignores and identities cannot alter
results; the reviewer name becomes the commit author.

## Feature root resolution

`--feature-root` flag, then `CODE_GRAPHER_DATA_DIR`, then
`${XDG_DATA_HOME}/code-grapher`, then `${HOME}/.local/share/code-grapher`.

## CLI

```text
cg-feature repo-id GIT_URL
cg-feature list GIT_URL
cg-feature status GIT_URL
cg-feature diff GIT_URL [FEATURE_NAME]
cg-feature draft GIT_URL SOURCE_JSON --expected-feature-name NAME
cg-feature approve GIT_URL FEATURE_NAME --reviewer NAME --note TEXT --expected-hash HASH
```

`draft` prints `DRAFT <rel_path>` or `NOOP` (byte-identical refresh) and never
commits. `approve` verifies `--expected-hash` against the current feature.json
sha256 before committing.

## REST API

See `docs/api.md` → “Features”. Routes are addressed by repo_id (the sha256
hex), not the database ID; repo responses expose a computed
`feature_repo_id` (null when `git_url` is missing or unparseable, and the web
UI then shows 该仓库未配置可用 git_url，无法使用功能评审).

## Scheduler usage

```sh
scripts/feature-periodic.sh GIT_URL FEATURE_NAME SOURCE_JSON [FEATURE_ROOT]
```

It only refreshes the draft (delegating to `cg-feature draft
--expected-feature-name`), stops on `NOOP`, and never approves or commits:
stable enters exclusively through human review (web 审核页 or
`cg-feature approve`).
