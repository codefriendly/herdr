# Codefriendly Herdr maintenance

This fork builds personal Herdr releases from upstream stable tags plus selected
patch sets. It does not follow unreleased `upstream/master` for personal builds.
Keeping each feature isolated preserves the option of preparing an upstream PR,
subject to upstream's contributor policy.

## Status

The published personal release is
[Codefriendly Herdr v0.9.4 — revision 1](https://github.com/codefriendly/herdr/releases/tag/codefriendly-v0.9.4-r1),
published October 13, 2026 as latest and non-prerelease, at source/tag commit
`c236b66d03d3ac0c0bd044287e1599d46596c364`. Its
[hosted workflow](https://github.com/codefriendly/herdr/actions/runs/38461083737)
built and packaged all five release targets. Maintenance/workflow source was
`2ca719fd047121d27ae329b13cf7f1a6b6a27429`. Build success does not attest to
manual hover or full local release validation.

Local preparation now targets upstream stable `v0.9.5`
(`2ecf19a92fc3a7fff75b6ce3655ef04f4625ec01`). The independently maintained
`pane-hover-focus` source is `40cff2ee5b050a6cad0e9b2747cc1134984c782e`.
The reconstructed `integration/codefriendly-release` source is
`78b1b52b6f57ba63a818519e06e6a878d06bb430`. Both have tree
`80e190bbb431131b8249af2e71fcead9002a8106`. A single consolidated export in
`fork/patches/pane-hover-focus/` reproduces this tree from the exact stable base.
Applying it independently in the integration worktree produces the same tree.
The profile contains only hover; no other feature is included. Upstream v0.9.5
does not add hover focus, and the v0.9.4 patch applies without conflicts.

Maintenance is prepared separately on `maintenance/codefriendly-release`, with
feature implementation absent from that source; fork `master` carries the same
maintenance source. The local rolling branches have been deliberately promoted
onto v0.9.5, preserving their previous v0.9.4 tips as `backup/master-v0.9.4`,
`backup/maintenance-v0.9.4`, `backup/pane-hover-focus-v0.9.4`, and
`backup/codefriendly-release-v0.9.4`. Earlier v0.9.3 backups remain intact.
No v0.9.5 source or maintenance candidate has been pushed or published.

The v0.9.5 feature source passed formatting, Clippy, 4,081 Rust tests with
15 skipped (including 59 hover-focused tests), maintenance/integration checks,
docs checks, release-docs checks, the 1/16-pane hover profile, and render scaling. The 16-pane hover handler
medians were 0.245 us/event with immediate replies and 0.155 us/event with
delayed replies; the corresponding 1-pane medians were both 0.072 us/event.
The render pipeline median was 623/624 us at 1/15 background workspaces
(1.00x) and 650/702 us at 1/15 active panes (1.08x). These are supporting
measurements, not timing gates. The release build succeeded and reports
`herdr 0.9.5`. `just check` stops at Windows cross-lint because this machine
has no configured Windows SDK. Release CPU smoke passed against the verified
official v0.9.5 binary, including two rounds of 60-second samples: hidden output
used 3.45% CPU versus 3.40% upstream (+1.5%); visible output was 0.25% for both.
Manual TUI hover validation remains pending. Inherited unfocused-window and
toast hover edge cases remain outside this port. No new binary was installed.

The separate maintenance candidate passed 22 offline fork-release tests,
formatting/Clippy, 4,030 Rust tests with 14 skipped, maintenance/integration
checks, docs checks, and release-docs checks. Those Rust results cover unpatched
v0.9.5 plus fork infrastructure, not hover. Its `just check` stops at
the same missing Windows SDK gate.

The fork release workflow now posts the personal-build announcement to devnull
only after publication succeeds. It uses the exact release tag and derives the
upstream version from that tag. `DEVNULL_SSH_KEY` must be configured as a
repository Actions secret; no live announcement was made during preparation.

The v0.9.1 release remains historical: it used native glibc Linux x86_64 and
Apple Silicon macOS binaries, not the five-target hosted asset set. Old v0.9.1
hover/integration tips remain preserved under their existing backup branches.

The original feature commit `3b0d3dc6`, based on
`d2cb0961663582a5069b1528edb93a9a8b23e1bc`, remains a historical reference;
it did not apply cleanly to `v0.9.0` after upstream's input/runtime reorganization.
The v0.9.0 release source subsequently passed all five hosted optimized release
builds; that is build evidence, not a replacement for manual smoke testing a new
source.

Previous published release:

- [Codefriendly Herdr v0.9.0 — revision 1](https://github.com/codefriendly/herdr/releases/tag/codefriendly-v0.9.0-r1)
  was published as latest and non-prerelease; later releases supersede it.
- Source/tag commit: `7a7023194477e003adbb7d8dc1a0b86095104257`.
- Maintenance/workflow commit: `9a4ce950`.
- [Successful release run](https://github.com/codefriendly/herdr/actions/runs/34386141152):
  resolve, all five platform builds, and publication passed.
- All five published assets were downloaded and passed `SHA256SUMS` verification.
  The Windows ZIP passed its integrity check and includes the app-local ConPTY
  runtime. Verification did not install or execute these downloaded binaries.

Earlier preflight failures were addressed by canonical upstream tag fetching and
repository identity/read-access checks without optional permission-field gates.
Both Linux builders then encountered a Google Chrome apt-index hash mismatch;
the workflow now excludes that unused repository before updating apt, leaving
preinstalled Chrome untouched. These fixes are included in the successful run.

Cache-action maintenance: commit `18ff0589` upgraded `actions/cache` to v6.1.0
and `Swatinem/rust-cache` to v2.9.2, both native Node 24, in normal CI and the fork
release workflow. [CI run 34401146078](https://github.com/codefriendly/herdr/actions/runs/34401146078)
passed Linux, macOS, Windows, ConPTY packaging, and commit-subject checks. Cache
runtime warnings and the `punycode`/`url.parse()` deprecations were absent. The
updated fork release workflow subsequently ran for v0.9.3.

The published v0.9.0 workflow used `mlugg/setup-zig` v2.2.1 and retained its
forced-Node-24 warning. The v0.9.5 candidate uses upstream's
`vercel-labs/setup-zig` v1.0.2 with Zig 0.16.0. Compared with v0.9.4, v0.9.5
keeps the same Rust/Zig versions, release optimization, target matrix, and
packaging scripts. The fork's Rust cache action matches v0.9.5's pinned action.
No hosted build has run for the v0.9.5 source.

Always verify upstream releases when selecting a future base; the release
workflow checks the explicitly recorded tag, never follows `latest` or
`upstream/master`.

## Remotes and branch roles

- `origin`: `https://github.com/codefriendly/herdr.git` — personal fork.
- `upstream`: `https://github.com/herdrdev/herdr.git` — official repository.
- Fork `master`: stable upstream base plus fork-only CI, maintenance documentation,
  exported patch sets, and build-profile metadata. It is the fork's default
  branch, making the manual release workflow available.
- Feature branches such as `pane-hover-focus`: persistent, independently
  maintained branches containing only that feature's implementation, tests, and
  relevant upstream-facing documentation. No fork infrastructure.
- `integration/codefriendly-release`: the exact personal-build source. Recreate
  it from the selected stable tag and apply every enabled feature in
  `codefriendly.series` order, even when the profile currently contains only one
  feature. Push its exact validated commit before hosted builds.
- The manual workflow lives on fork `master` but checks out the immutable source
  SHA recorded for `integration/codefriendly-release`. Never build fork `master`
  with its maintenance material.
- `pr/<feature>`: current **`upstream/master`** plus only the selected feature,
  adapted and validated against that development base.

Stable tags are explicit update inputs, not Git tracking branches. Ordinary
`git pull` must not silently advance stable-based fork branches onto upstream
unreleased development. Fork `master`, each enabled feature branch, and
`integration/codefriendly-release` advance deliberately onto the same selected
stable tag. Preserve previous published tips before rewriting rolling branches.
A tag identifies a commit just like a branch: conflicts use normal Git
resolution, followed by `git add` and `git rebase --continue`. Fetching
`upstream/master` keeps it available for inspection without using it as the
personal build base.

## Fork master layout

```text
fork/
├── README.md
├── release.py
├── test_release.py
├── upstream-base
├── profiles/
│   ├── codefriendly.series
│   └── codefriendly.toml
└── patches/
    └── pane-hover-focus/
        ├── metadata.toml
        └── 0001-feat-port-pane-hover-focus-to-herdr-v0.9.5.patch
```

`upstream-base` records the selected stable tag and its resolved full commit ID.
Each patch set's metadata records its name, base tag and commit, independent
source branch and exact source commit, ordered patch filenames, and any
dependencies on other sets. `profiles/codefriendly.series` lists enabled patch
sets in integration order. `profiles/codefriendly.toml` records the exact
`integration/codefriendly-release` commit and tree produced by that profile.

Keep unrelated features in separate sets. A set may contain multiple commits.
Prefer independent sets based on the same stable tag; explicitly record any
unavoidable dependency rather than hiding it in an exported range.

### Source of truth and reproducibility

Develop in clean feature branches. Generate patch files with `git format-patch`;
do not maintain a second implementation by hand-editing exported patches.
Archive the generated files and metadata together on fork `master` after each
validated update. Existing release tags remain immutable.

Before accepting an export:

1. Apply each feature's ordered patches in a disposable checkout at the exact
   recorded base, including declared dependencies when applicable.
2. Verify that each independent export reproduces its recorded feature source
   tree.
3. Recreate `integration/codefriendly-release` from the recorded base, apply all
   enabled sets in `codefriendly.series` order, and verify that its exact tree
   equals `codefriendly.toml`. Always perform this integration step, including
   for a one-feature profile; individually valid patches can still conflict or
   interact as the profile grows.

Do not label an old-base export as compatible with a new stable tag before this
verification succeeds.

## Stable update procedure

1. Start with clean worktrees and fetch upstream and fork references. Verify the
   latest published non-prerelease upstream release and resolve its tag to a
   full commit ID. Preserve the previous released source and patch metadata.
2. Check whether upstream now supplies any selected feature. Retire redundant
   patches deliberately rather than layering duplicate behavior onto it.
3. Port each feature independently on its persistent feature branch and stable
   worktree—for example, `pane-hover-focus` in
   `../herdr-worktrees/pane-hover-focus`. Use
   `integration/codefriendly-release` in
   `../herdr-worktrees/codefriendly-release` for profile composition. These names
   describe branch roles rather than a particular upstream version and remain
   stable across release updates. Use a separate maintenance worktree when
   rebasing fork infrastructure and regenerating exports. Preserve unrelated
   work; do not blindly resolve conflicts by accepting all of either side.
4. Follow repository risk classification, review, and test requirements. Run
   `just check` and relevant manual behavior tests; document failures or any
   explicitly accepted narrower validation. Patch application alone is not
   behavioral verification.
5. Regenerate each feature export and update its base/source/dependency
   metadata. Recreate `integration/codefriendly-release` from the selected stable
   tag, apply every enabled set in profile order, and record its exact commit and
   tree in `codefriendly.toml`. Verify both independent feature exports and the
   integrated profile tree.
6. Validate the selected release source and manually test pane hover with the setting on
   and off, mouse capture, pane switching, and application mouse behavior as
   relevant to the port. Follow performance guidance for pane-scaled input work.
7. Refresh fork infrastructure separately against the same stable tag. Compare
   the fork build workflow with the stable tag's toolchains, optimization,
   packaging, and target requirements; do not assume old CI remains equivalent.
8. Propose commit messages and obtain alignment before committing, as required
   by the repository. Branch rewrites, pushes, and publication require explicit
   authorization. When an agreed rolling branch must be rewritten, preserve its
   old tip and use a verified `--force-with-lease`, never an unconditional force.
9. Push the exact validated `integration/codefriendly-release` commit, then
   manually dispatch the fork release workflow from fork `master` with that full
   SHA. Do not publish an unverified moving branch tip.

## Manual fork workflow

After separately authorizing and completing commits/pushes, dispatch
`.github/workflows/fork-release.yml` in `codefriendly/herdr` from **master**:

- `source_sha`: the full lowercase integration commit recorded in
  `fork/profiles/codefriendly.toml` (currently
  `78b1b52b6f57ba63a818519e06e6a878d06bb430`), not a branch/tag.
- `revision`: a previously unused positive integer without leading zeros. The
  first Codefriendly release based on `v0.9.5` uses revision `1`.

The workflow reads metadata and `fork/release.py` from the immutable dispatched
maintenance/workflow commit. It checks the upstream GitHub Release is published
and not draft/prerelease, peels that exact tag to the recorded commit, then
fetches only `refs/tags/<recorded-tag>` from canonical `herdrdev/herdr` with
`--no-tags` and verifies `FETCH_HEAD^{commit}` matches that same base. Fork-local
tags are neither trusted nor overwritten. It checks stable-base ancestry and
source tree/Cargo identity, and applies the ordered
archive to an isolated Git index to prove tree equivalence. Missing API access,
metadata, source history, or stable-release evidence fails closed. Ancestry or
patch equivalence alone does not prove behavioral correctness.

All five targets build the same resolved integration SHA directly. Rust `1.96.1`,
Zig `0.16.0`, ReleaseFast/SIMD, Linux musl/linker settings, and Windows ConPTY
packaging follow `v0.9.5` release build settings.
The workflow does not run local full checks, benchmarks, manual smoke tests,
Nix checks, or upstream docs/distribution release checks. Keep detailed local
validation results and skipped checks in maintenance records, not in the
published release notes. Public notes report verified CI builds and packaging
without a checklist of unperformed manual testing. Do not attribute historical
local results to a new source or imply upstream endorsement.

Fork access checks require successful repository metadata and release-list API
reads, with the returned owner/repository matching `codefriendly/herdr` exactly
apart from ASCII letter case (GitHub repository names are case-insensitive).
They do not rely on the optional user-token-style `permissions.pull` field:
Actions uses its own `GITHUB_TOKEN`, not the local user's `gh` token. Metadata
success alone does not prove write access or draft visibility. API failures stop
the check; only an explicit 404 for the candidate tag GET means that tag is absent.
Malformed metadata and identity mismatches have separate diagnostics without
logging tokens or the complete API response.

Both resolution and publication reject an existing tag or release, including
returned drafts. Publication is serialized by the resolved release tag and atomically
creates a new tag before creating its own draft and uploading assets without
replacement. Only that newly created draft is promoted to non-prerelease/latest.
A race or partial failure stops the run; a reserved tag/draft is **not** cleaned
up or reused automatically. Inspect the failure and use a new revision for a
new authorized attempt. GitHub repository writers can still edit releases
outside this workflow; this workflow never overwrites them.

Offline helper checks (no GitHub requests):

```bash
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s fork -p 'test_release.py' -v
```

These require Python 3.11+ and the recorded source/base Git objects locally.

## Release identity

Distribution name: **Codefriendly Herdr**. Feature names belong in the patch
inventory, not in the distribution's tag name.

```text
Tag:   codefriendly-v0.9.5-r1
Title: Codefriendly Herdr v0.9.5 — revision 1
```

A revised patch/build on the same upstream base becomes
`codefriendly-v0.9.5-r2`. A new upstream stable base starts at revision 1, for
example `codefriendly-v0.9.6-r1`. Never overwrite an existing release's source or
assets to represent a different build.

The release body records:

- Upstream stable tag and full commit ID.
- Exact patched source commit.
- Selected patch sets and their revisions/source commits.
- Verified CI build and packaging results, plus any concrete known user-facing issues.
  Keep skipped-check and manual-testing details in internal maintenance records,
  not in published release notes.
- Installation and update instructions.

Keep the existing portable asset names:

```text
herdr-linux-x86_64
herdr-linux-aarch64
herdr-macos-x86_64
herdr-macos-aarch64
herdr-windows-x86_64.zip
SHA256SUMS
```

Windows archives include the app-local ConPTY runtime. Publish successful
personal releases as non-prereleases and explicitly mark the intended release
latest so the existing checksum-verifying Codefriendly installer can discover
it. Stable-based does not imply upstream endorsement or upstream-equivalent
validation.

Keep the upstream Cargo version unless a separate binary-identity change is
agreed. Thus `herdr --version` may say `0.9.5`; the release tag and recorded source
commit distinguish the fork revision. Update personal installations through the
Codefriendly installer/mytools, not the upstream Herdr updater, which may replace
the patched binary with an official build.

## Preparing a potential upstream PR

Never create the PR branch from fork `master` or submit the combined personal
build. The PR base is current **`upstream/master`**, even though personal builds
use stable tags.

1. Read the current upstream `AGENTS.md` and `CONTRIBUTING.md`. Before pushing or
   opening a PR, verify the acting GitHub account and follow upstream's intake
   rules. Interest in a feature is not a waiver: an external implementation PR
   requires the authenticated human to be in `.github/APPROVED_CONTRIBUTORS`
   under the current policy. Do not open a feature-request issue as a workaround.
2. Export/copy the selected patch files and metadata **out of fork master into a
   separate temporary directory before switching branches**. The `fork/` tree
   deliberately does not exist on upstream-based feature/PR branches. Select only
   the requested set and review any dependencies.
3. Fetch upstream and create a clean branch:

   ```bash
   git fetch upstream
   git switch --create pr/pane-hover-focus upstream/master
   ```

4. Apply the saved files in their recorded order:

   ```bash
   git am -3 /absolute/path/to/exported/0001-focus-panes-on-hover.patch
   ```

   Three-way application is a convenience, not proof of compatibility. Resolve
   drift, adapt the implementation to current upstream architecture, and recheck
   whether upstream already provides the behavior.
5. Run `just check`, relevant manual tests, and required review/performance checks.
   Inspect the complete diff from `upstream/master`: it must contain only the
   intended feature, tests, and relevant documentation—no fork CI, release
   metadata, exported patches, or unrelated customizations.
6. Only when authorized and eligible under upstream policy, push the clean PR
   branch to the fork and open a PR targeting `herdrdev/herdr:master`. Otherwise
   retain the branch locally as prepared material. Never merge on behalf of
   upstream maintainers.

Adapting a patch for a PR does not automatically change the stable-based personal
release. Any useful changes must be deliberately backported, re-exported, and
validated against the recorded stable base.
