# Git-backed feature review (MR branch model, git_url identity)

Feature lifecycle state comes **only** from git. There is no `feature.json`
schema, no version directories, no `index.json`, and the database is not
involved: no rows, no autoincrement IDs, no name mapping. Every source
repository's understanding output is **one independent git document repo**
living on the feature root (typically an NFS mount).

## Identity

- Logical repository identity: the **canonical git_url**.
- Physical directory name: `repo_id = sha256(canonical_git_url)` (64 lowercase
  hex characters). URLs are never stored as paths, so credentials, illegal
  characters, traversal, and length are non-issues.
- MR identity: `name` — the only ID; doubles as the branch suffix and the
  docs namespace directory.

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
  3f9c1e5a…/                          # repo_id = sha256(canonical git_url)
    (git repo)                         # one independent git repo per source repo
    main                               # merged stable state
    mr/AST解析与领域模型                # draft branch = one MR per feature
    mr/请求链路推断
    docs/                              # arbitrary text document tree
      AST解析与领域模型/overview.md
      请求链路推断/index.json
    _changelog/
      2026-11-19-AST解析与领域模型-a1b2c3.md
```

- Each MR owns exactly one namespace: `docs/<name>/`. Draft refresh mirrors
  the source tree **into that namespace only** — other MRs' merged docs and
  `_changelog/` are never touched (approving an MR can never delete another
  MR's files).
- Files are UTF-8 text (`.md`, `.json`, …); the API tags each with a render
  hint (markdown / json / text) and caps blob reads at 512 KB.

### MR name rules

Allowed: unicode text (Chinese included), letters, digits, `-`, `_`, `.`.
Rejected: empty, `.`/`..`, `/`, whitespace, control characters, git ref
metacharacters (`~ ^ : ? * [ \\`), `..` sequences, trailing `.lock`, leading
`.` or `/`, the `_`-prefixed reserved namespace (`_changelog`), names over
128 bytes. Names are case-sensitive: `AST` and `ast` are two MRs.

## Lifecycle (branch semantics)

```text
mr/<name> tip ∉ main   -> open (draft)
mr/<name> tip ∈ main   -> merged
draft  = mirror files onto mr/<name> and commit (branch may be reset/rebuilt)
approve = git merge --no-ff mr/<name> into main (reviewer = merge author)
          + append _changelog/<date>-<name>-<merge6>.md on main
```

- Draft refresh first absorbs `main` into the branch; on conflict it resets
  the branch to `main` and re-mirrors (drafts are pure functions of their
  source files and fully rebuildable).
- Identical content refresh prints `NOOP` and creates no commit.
- Merging only ever adds the MR's own namespace; conflicts at approve time
  mean the draft is stale — refresh it first.
- MRs are never deleted: a re-drafted MR reopens the branch, and every merge
  adds one changelog entry.

## Concurrency (NFS)

- All mutations (`draft`, `approve`) take `flock(<repo_id>/.lock)` for their
  whole critical section, inside cg-feature. **One writer per partition**:
  the cg-server instance owning the feature root. Do not run writers from
  multiple machines.
- Reads (`mrs`, `tree`, `blob`, `diff`, `commits`, `changelog`) resolve
  through git refs (`ls-tree` / `show` / `log`) — lock-free and never
  half-written.
- All git invocations run with `GIT_CONFIG_GLOBAL=/dev/null` and
  `GIT_CONFIG_NOSYSTEM=1` and `core.quotepath=false`, so results never depend
  on the invoking machine's config.

## Feature root resolution

`--feature-root` flag, then `CODE_GRAPHER_DATA_DIR`, then
`${XDG_DATA_HOME}/code-grapher`, then `${HOME}/.local/share/code-grapher`.

## CLI

```text
cg-feature repo-id GIT_URL
cg-feature mrs GIT_URL
cg-feature tree GIT_URL [--mr NAME]          # NAME omitted = main
cg-feature cat GIT_URL PATH [--mr NAME]
cg-feature diff GIT_URL FEATURE_NAME
cg-feature commits GIT_URL FEATURE_NAME
cg-feature draft GIT_URL FEATURE_NAME SOURCE_DIR [--expected-feature-name NAME]
cg-feature approve GIT_URL FEATURE_NAME --reviewer NAME [--note TEXT] [--expected-tip HASH]
cg-feature changelog GIT_URL
```

`draft` mirrors SOURCE_DIR into `docs/<FEATURE_NAME>/` on branch
`mr/<FEATURE_NAME>`, prints `DRAFT <commit>` or `NOOP`, and never approves.
`approve` merges the branch into `main` as the reviewer.

## REST API

See `docs/api.md` → “Features”. Routes are addressed by repo_id (the sha256
hex), not the database ID; repo responses expose a computed
`feature_repo_id` (null when `git_url` is missing or unparseable, and the web
UI then shows 该仓库未配置 Git URL，仓库理解不可用 — 补填 Git URL 或改选 URL 注册的仓库).

## Scheduler usage

```sh
scripts/feature-periodic.sh GIT_URL FEATURE_NAME SOURCE_DIR [FEATURE_ROOT]
```

It only refreshes the draft branch (delegating to `cg-feature draft
--expected-feature-name`), stops on `NOOP`, and never approves or commits:
stable enters exclusively through human review (web 评审页 or `cg-feature
approve`).

## Migrating from v1

The v1 single-branch `feature.json` layout is not migrated: it held bootstrap
data only. Start fresh in the new layout (delete or archive the old
`<repo_id>/` directories if needed).
