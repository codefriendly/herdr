"""Single-profile Codefriendly release checks and create-only publication (Python 3.11+)."""

import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib
import urllib.error
import urllib.parse
import urllib.request

REPOSITORY = "codefriendly/herdr"
UPSTREAM = "herdrdev/herdr"
ASSETS = (
    "herdr-linux-x86_64",
    "herdr-linux-aarch64",
    "herdr-macos-x86_64",
    "herdr-macos-aarch64",
    "herdr-windows-x86_64.zip",
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def git(*args, env=None):
    return subprocess.check_output(["git", *args], text=True, env=env).strip()


def api(path, *, method="GET", data=None, missing=False, binary=False):
    host = "https://uploads.github.com" if binary else "https://api.github.com"
    headers = {
        "Authorization": f"Bearer {os.environ['GH_TOKEN']}",
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
    }
    if data is not None:
        headers["Content-Type"] = "application/octet-stream" if binary else "application/json"
        if not binary:
            data = json.dumps(data).encode()
    request = urllib.request.Request(host + path, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        status = error.code
        error.close()
        if missing and method == "GET" and status == 404:
            return None
        raise ValueError(f"GitHub {method} {path} failed: HTTP {status}") from None


def dispatch_context():
    require(os.environ["GITHUB_REPOSITORY"] == REPOSITORY, "Wrong fork repository")
    require(os.environ["GITHUB_EVENT_NAME"] == "workflow_dispatch", "Manual dispatch required")
    require(os.environ["GITHUB_REF"] == "refs/heads/master", "Dispatch from fork master only")
    workflow = os.environ["WORKFLOW_SHA"]
    require(re.fullmatch(r"[0-9a-f]{40}", workflow), "Invalid workflow SHA")
    require(workflow == os.environ["GITHUB_SHA"] == git("rev-parse", "HEAD"),
            "Maintenance checkout must equal the immutable dispatched workflow commit")
    return workflow


def metadata():
    source = os.environ["SOURCE_SHA"]
    revision = os.environ["REVISION"]
    require(re.fullmatch(r"[0-9a-f]{40}", source), "Source must be a full lowercase commit SHA")
    require(re.fullmatch(r"[1-9][0-9]*", revision), "Revision must be a positive integer without leading zeros")
    base = tomllib.loads(Path("fork/upstream-base").read_text())
    require(re.fullmatch(r"v[0-9]+\.[0-9]+\.[0-9]+", base["tag"]), "Base must be a stable version tag")
    require(re.fullmatch(r"[0-9a-f]{40}", base["commit"]), "Invalid base commit")
    profile = Path("fork/profiles/codefriendly.series").read_text().splitlines()
    require(profile == ["pane-hover-focus"], "Only the single pane-hover-focus profile is supported")
    patch_dir = Path("fork/patches/pane-hover-focus")
    patch = tomllib.loads((patch_dir / "metadata.toml").read_text())
    require(patch["name"] == patch["source_branch"] == "pane-hover-focus", "Unexpected patch set")
    require(patch["base_tag"] == base["tag"] and patch["base_commit"] == base["commit"], "Patch base mismatch")
    require(patch["source_commit"] == source, "Source does not match archived metadata")
    require(re.fullmatch(r"[0-9a-f]{40}", patch["source_tree"]), "Invalid source tree")
    require(patch["dependencies"] == [], "Patch dependencies are not supported")
    filenames = patch["patches"]
    require(isinstance(filenames, list) and filenames and len(set(filenames)) == len(filenames), "Invalid patch inventory")
    for filename in filenames:
        require(re.fullmatch(r"[0-9]{4}-[a-zA-Z0-9_.-]+\.patch", filename), "Invalid patch filename")
        require((patch_dir / filename).is_file() and not (patch_dir / filename).is_symlink(), "Missing or linked patch")
    require(set(filenames) == {p.name for p in patch_dir.glob("*.patch")}, "Unlisted archived patch")
    return base, patch, f"codefriendly-{base['tag']}-r{revision}"


def check_available(tag):
    # Authenticate/read the repository first: an inaccessible repo must not look absent.
    repo = api(f"/repos/{REPOSITORY}")
    require(repo["full_name"] == REPOSITORY and repo["permissions"]["pull"] is True, "Cannot read fork")
    require(api(f"/repos/{REPOSITORY}/git/ref/tags/{tag}", missing=True) is None, "Release tag already exists")
    # Include drafts, which the release-by-tag endpoint may not return.
    page = 1
    while True:
        releases = api(f"/repos/{REPOSITORY}/releases?per_page=100&page={page}")
        require(isinstance(releases, list), "Invalid release listing")
        require(not any(release["tag_name"] == tag for release in releases), "Release already exists (including draft)")
        if len(releases) < 100:
            break
        page += 1


def verify_upstream(base):
    release = api(f"/repos/{UPSTREAM}/releases/tags/{base['tag']}")
    require(release["tag_name"] == base["tag"] and release["draft"] is False
            and release["prerelease"] is False and release["published_at"], "Base is not a published upstream stable release")
    ref = api(f"/repos/{UPSTREAM}/git/ref/tags/{base['tag']}")
    obj = ref["object"]
    for _ in range(10):
        if obj["type"] == "commit":
            break
        require(obj["type"] == "tag", "Unexpected upstream tag object")
        obj = api(f"/repos/{UPSTREAM}/git/tags/{obj['sha']}")["object"]
    require(obj["type"] == "commit" and obj["sha"] == base["commit"], "Upstream stable tag does not peel to the recorded base")


def verify_source(base, patch):
    source = patch["source_commit"]
    require(git("rev-parse", f"{base['tag']}^{{commit}}") == base["commit"], "Local stable tag mismatch")
    require(git("rev-parse", f"{source}^{{commit}}") == source, "Source is not a commit")
    for tip in (source, "HEAD"):
        subprocess.run(["git", "merge-base", "--is-ancestor", base["commit"], tip], check=True)
    require(git("rev-parse", f"{source}^{{tree}}") == patch["source_tree"], "Source tree mismatch")
    cargo = tomllib.loads(git("show", f"{source}:Cargo.toml"))
    require(cargo["package"]["version"] == base["tag"][1:], "Source Cargo version differs from stable base")
    # Reconstruct only the tree in an isolated index; never execute feature scripts.
    # Unlike ancestry alone, this proves the archive describes every source-tree change.
    with tempfile.TemporaryDirectory(prefix="fork-release-") as temp:
        env = dict(os.environ, GIT_INDEX_FILE=str(Path(temp) / "index"))
        git("read-tree", base["commit"], env=env)
        for filename in patch["patches"]:
            git("apply", "--cached", str(Path("fork/patches/pane-hover-focus") / filename), env=env)
        require(git("write-tree", env=env) == patch["source_tree"], "Archived patches do not reproduce the source tree")


def notes(base, patch, workflow):
    inventory = "\n".join(
        f"- `{filename}` — SHA-256 `{hashlib.sha256((Path('fork/patches/pane-hover-focus') / filename).read_bytes()).hexdigest()}`"
        for filename in patch["patches"]
    )
    return f"""Personal Codefriendly Herdr build; not an upstream release or endorsement.

## Provenance
- Upstream stable: `{base['tag']}` at `{base['commit']}` (herdrdev/herdr).
- Patched source: `{patch['source_commit']}`; tree `{patch['source_tree']}`.
- Profile: `codefriendly`; patch set: `{patch['name']}`; source branch label: `{patch['source_branch']}` (not resolved at build time).
- Maintenance/workflow commit: `{workflow}` in codefriendly/herdr.
- Workflow: `.github/workflows/fork-release.yml`; run: https://github.com/{REPOSITORY}/actions/runs/{os.environ['GITHUB_RUN_ID']} (attempt {os.environ['GITHUB_RUN_ATTEMPT']}).

Ordered archived patches from that maintenance commit:
{inventory}

## Validation scope
Automated for this source: published non-prerelease upstream tag and exact peeled base, base ancestry, recorded source tree and Cargo version, and ordered archived-patch tree equivalence. All five release-target builds and packaging jobs succeeded before publication; SHA256SUMS covers the five assets. Windows packaging includes the app-local ConPTY runtime and checks binary version identity.

This workflow does not run `just check`, render/performance benchmarks, manual pane-hover/binary smoke tests, Nix checks, or upstream release-docs/distribution checks. It does not attest to earlier local testing or imply upstream-equivalent validation. Build success and patch equivalence are not behavioral verification.

## Install / update
Use the checksum-verifying **Codefriendly installer/mytools** for this personal release. Do not use the upstream Herdr updater: it may replace the patched binary with an official build. Cargo/binary version remains `{base['tag'][1:]}`; the Codefriendly release tag identifies the fork revision.
"""


def prepare_assets():
    destination = Path("release-assets")
    destination.mkdir()
    files = [p for p in Path("artifacts").rglob("*") if p.is_file()]
    require(len(files) == len(ASSETS) and {p.name for p in files} == set(ASSETS), "Unexpected, duplicate, or missing build assets")
    for path in files:
        require(not path.is_symlink() and path.stat().st_size > 0, "Invalid build asset")
        (destination / path.name).write_bytes(path.read_bytes())
    sums = "".join(f"{hashlib.sha256((destination / name).read_bytes()).hexdigest()}  {name}\n" for name in sorted(ASSETS))
    (destination / "SHA256SUMS").write_text(sums)
    return destination


def publish(base, patch, tag, workflow):
    require(os.environ["RESOLVED_SHA"] == patch["source_commit"] and os.environ["RESOLVED_TAG"] == tag, "Resolved identity changed")
    assets = prepare_assets()
    body = notes(base, patch, workflow)
    verify_upstream(base)
    check_available(tag)
    # POST ref is the atomic reservation: a racing tag creator gets a 422, never an update.
    # Any later failure leaves the tag/draft reserved. Retry only with a new revision.
    api(f"/repos/{REPOSITORY}/git/refs", method="POST", data={"ref": f"refs/tags/{tag}", "sha": patch["source_commit"]})
    release = api(f"/repos/{REPOSITORY}/releases", method="POST", data={
        "tag_name": tag, "target_commitish": patch["source_commit"],
        "name": f"Codefriendly Herdr {base['tag']} — revision {os.environ['REVISION']}",
        "body": body, "draft": True, "prerelease": False,
    })
    release_id = release["id"]
    require(type(release_id) is int and release["draft"] is True and release["tag_name"] == tag, "Unexpected release creation response")
    for name in (*ASSETS, "SHA256SUMS"):
        # Upload by newly created release ID, with no clobber/delete/replacement path.
        api(f"/repos/{REPOSITORY}/releases/{release_id}/assets?name={urllib.parse.quote(name)}", method="POST", data=(assets / name).read_bytes(), binary=True)
    ref = api(f"/repos/{REPOSITORY}/git/ref/tags/{tag}")
    require(ref["object"]["type"] == "commit" and ref["object"]["sha"] == patch["source_commit"], "Reserved tag changed during publication")
    api(f"/repos/{REPOSITORY}/releases/{release_id}", method="PATCH", data={"draft": False, "make_latest": "true"})


def main():
    require(len(sys.argv) == 2 and sys.argv[1] in ("resolve", "publish"), "Expected resolve or publish")
    workflow = dispatch_context()
    base, patch, tag = metadata()
    if sys.argv[1] == "resolve":
        verify_upstream(base)
        verify_source(base, patch)
        check_available(tag)
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write(f"commit={patch['source_commit']}\ntag={tag}\n")
    else:
        publish(base, patch, tag, workflow)


if __name__ == "__main__":
    main()
