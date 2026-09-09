# Codefriendly Herdr maintenance

This fork builds personal Herdr releases from upstream stable tags plus selected
patch sets. It does not follow unreleased `upstream/master` for personal builds.
Keeping each feature isolated preserves the option of preparing an upstream PR,
subject to upstream's contributor policy.

## Status

The stable-tag migration is committed and published to the personal fork.
Fork `master` has been rebased onto selected upstream stable `v0.9.0`
(`b99002ac99b09e00b4ca692436cb15a6b0d676f1`), with fork CI commits `2ac91ce9`
and `2737bb07`. The feature branch is separately ported to the same base at
`7a7023194477e003adbb7d8dc1a0b86095104257`; its three ordered exports and exact
tree are recorded under `fork/patches/pane-hover-focus/`. Export reconstruction
has matched that source tree. Feature implementation remains absent from fork
`master` by design.

The original feature commit `3b0d3dc6`, based on
`d2cb0961663582a5069b1528edb93a9a8b23e1bc`, remains a historical reference;
it did not apply cleanly to `v0.9.0` after upstream's input/runtime reorganization.
The final simplified source passed local full checks (3312 passed, 3 skipped),
benchmarks, and independent review. The last release-binary/manual smoke test
covered the previous `1e627856` source, **not** the final `7a702319` source.
The final source subsequently passed all five hosted optimized release builds;
that is build evidence, not a replacement for a manual smoke test of that source.
Normal CI on fork maintenance commit `bd39ddcd` also passed; it tested fork
`master`, not the feature source.

Local `master` tracks `origin/master`, and `pane-hover-focus` tracks
`origin/pane-hover-focus`; both stable-based branches have been pushed. The fork
workflow, documentation/exports, and AGENTS fork appendix are committed.

Published release:

- [Codefriendly Herdr v0.9.0 — revision 1](https://github.com/codefriendly/herdr/releases/tag/codefriendly-v0.9.0-r1)
  is published as latest and non-prerelease.
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

Deferred maintenance: the pinned cache and Zig actions emit Node deprecation
warnings, including Node-20 actions forced onto Node 24. At the release audit,
`actions/cache` v6.1.0 and `Swatinem/rust-cache` v2.9.2 declared native Node 24;
latest `mlugg/setup-zig` v2.2.1 still declared Node 20. Review compatible pinned
upgrades separately; do not simply remove the Node 24 override. These warnings
did not block publication.

Always verify upstream releases when selecting a future base; the release
workflow checks the explicitly recorded tag, never follows `latest` or
`upstream/master`.

## Remotes and branch roles

- `origin`: `https://github.com/codefriendly/herdr.git` — personal fork.
- `upstream`: `https://github.com/herdrdev/herdr.git` — official repository.
- Fork `master`: stable upstream base plus fork-only CI, maintenance documentation,
  exported patch sets, and build-profile metadata. It is the fork's default
  branch, making the manual release workflow available.
- Feature branches such as `pane-hover-focus`: recorded stable base plus only
  that feature's implementation and tests, including relevant upstream-facing
  documentation. No fork infrastructure.
- Current release source: `pane-hover-focus` directly. With one patch set, no
  separate build branch is needed. The manual workflow lives on fork `master`
  but checks out the exact validated feature commit to build.
- Optional future combined build branch: needed only if a release combines
  multiple independent patch sets. Start from the recorded stable base and apply
  the Codefriendly profile in declared order. Do not build fork `master` with
  its maintenance material.
- `pr/<feature>`: current **`upstream/master`** plus only the selected feature,
  adapted and validated against that development base.

Local fork `master` and `pane-hover-focus` track their corresponding `origin`
branches. Stable tags are explicit update inputs, not Git tracking branches.
Ordinary `git pull` must not silently advance a stable-based fork onto upstream
unreleased development. Both fork `master` and feature branches advance
independently onto the same selected stable tag. Prefer rebasing their respective
fork-only commits, preserving previous tips before rewriting history. A tag
identifies a commit just like a branch: conflicts use normal Git resolution,
followed by `git add` and `git rebase --continue`. Fetching `upstream/master`
keeps it available for inspection without using it as the personal build base.

## Fork master layout

```text
fork/
├── README.md
├── release.py
├── test_release.py
├── upstream-base
├── profiles/
│   └── codefriendly.series
└── patches/
    └── pane-hover-focus/
        ├── metadata.toml
        ├── 0001-feat-focus-panes-on-hover.patch
        ├── 0002-fix-preserve-hover-focus-ordering-and-interaction-gu.patch
        └── 0003-refactor-simplify-hover-intent-and-test-setup.patch
```

`upstream-base` records the selected stable tag and its resolved full commit ID.
Each patch set's metadata records its name, base tag and commit, source branch
and exact source commit, ordered patch filenames, and any dependencies on other
sets. `profiles/codefriendly.series` lists enabled patch sets in application
order, initially just `pane-hover-focus`.

Keep unrelated features in separate sets. A set may contain multiple commits.
Prefer independent sets based on the same stable tag; explicitly record any
unavoidable dependency rather than hiding it in an exported range.

### Source of truth and reproducibility

Develop in clean feature branches. Generate patch files with `git format-patch`;
do not maintain a second implementation by hand-editing exported patches.
Archive the generated files and metadata together on fork `master` after each
validated update. Existing release tags remain immutable.

Before accepting an export:

1. Apply its ordered patches with `git am` in a disposable checkout at the exact
   recorded base (including declared dependencies, if any).
2. Verify that the resulting Git tree equals the recorded feature source tree.
3. For the current single-set profile, verify the export reproduces the exact
   `pane-hover-focus` commit tree used for release. If multiple sets are enabled
   later, apply the complete profile in order and validate the combined result;
   individually applicable patches can still conflict or interact.

Do not label an old-base export as compatible with a new stable tag before this
verification succeeds.

## Stable update procedure

1. Start with clean worktrees and fetch upstream and fork references. Verify the
   latest published non-prerelease upstream release and resolve its tag to a
   full commit ID. Preserve the previous released source and patch metadata.
2. Check whether upstream now supplies any selected feature. Retire redundant
   patches deliberately rather than layering duplicate behavior onto it.
3. Port each feature independently onto the chosen stable tag. Use dedicated
   worktrees for substantive ports. Preserve unrelated work; do not blindly
   resolve conflicts by accepting all of either side.
4. Follow repository risk classification, review, and test requirements. Run
   `just check` and relevant manual behavior tests; document failures or any
   explicitly accepted narrower validation. Patch application alone is not
   behavioral verification.
5. Regenerate exports and update base/source/dependency metadata. Verify their
   tree equivalence. Use the feature branch directly for the current single-set
   profile; synthesize a combined source only if multiple sets are selected.
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
9. Manually dispatch the fork release workflow from fork `master` for the exact
   validated source commit on `pane-hover-focus` (or a future combined source).
   Do not publish an unverified moving branch tip.

## Manual fork workflow

After separately authorizing and completing commits/pushes, dispatch
`.github/workflows/fork-release.yml` in `codefriendly/herdr` from **master**:

- `source_sha`: the full lowercase source commit in the archived metadata
  (currently `7a7023194477e003adbb7d8dc1a0b86095104257`), not a branch/tag.
- `revision`: a previously unused positive integer without leading zeros;
  the workflow input defaults to `1`, but that revision is already published for
  the current base. The next release on `v0.9.0` must use an unused revision
  (currently `2`), not the default.

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

All five targets build the same resolved feature SHA directly. Rust `1.96.1`,
Zig `0.15.2` (Homebrew `zig@0.15` on macOS), ReleaseFast/SIMD, Linux musl/linker
settings, and Windows ConPTY packaging follow `v0.9.0` release build settings.
The workflow does not run local full checks, benchmarks, manual smoke tests,
Nix checks, or upstream docs/distribution release checks. Release notes state
these limits rather than attributing historical local results to a new source.

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
Tag:   codefriendly-v0.9.0-r1
Title: Codefriendly Herdr v0.9.0 — revision 1
```

A revised patch/build on the same upstream base becomes
`codefriendly-v0.9.0-r2`. A new upstream stable base starts at revision 1, for
example `codefriendly-v0.9.1-r1`. Never overwrite an existing release's source or
assets to represent a different build.

The release body records:

- Upstream stable tag and full commit ID.
- Exact patched source commit.
- Selected patch sets and their revisions/source commits.
- Validation performed, skipped checks, and known risks.
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
agreed. Thus `herdr --version` may say `0.9.0`; the release tag and recorded source
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
