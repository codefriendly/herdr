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

    profile = tomllib.loads(Path("fork/profiles/codefriendly.toml").read_text())
    require(profile["name"] == "codefriendly", "Unexpected profile")
    require(profile["source_branch"] == "integration/codefriendly-release", "Unexpected integration branch")
    require(profile["base_tag"] == base["tag"] and profile["base_commit"] == base["commit"], "Profile base mismatch")
    require(profile["source_commit"] == source, "Source does not match profile metadata")
    require(re.fullmatch(r"[0-9a-f]{40}", profile["source_tree"]), "Invalid profile source tree")

    series = Path("fork/profiles/codefriendly.series").read_text().splitlines()
    require(series and len(set(series)) == len(series), "Invalid patch-set series")
    patches = []
    seen = set()
    for name in series:
        require(re.fullmatch(r"[a-z0-9][a-z0-9-]*", name), "Invalid patch-set name")
        patch_dir = Path("fork/patches") / name
        patch = tomllib.loads((patch_dir / "metadata.toml").read_text())
        require(patch["name"] == name, "Patch-set name mismatch")
        require(re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._/-]*", patch["source_branch"]), "Invalid patch source branch")
        require(patch["base_tag"] == base["tag"] and patch["base_commit"] == base["commit"], "Patch base mismatch")
        require(re.fullmatch(r"[0-9a-f]{40}", patch["source_commit"]), "Invalid patch source commit")
        require(re.fullmatch(r"[0-9a-f]{40}", patch["source_tree"]), "Invalid patch source tree")
        dependencies = patch["dependencies"]
        require(isinstance(dependencies, list) and all(dependency in seen for dependency in dependencies), "Patch dependencies must precede their dependent set")
        filenames = patch["patches"]
        require(isinstance(filenames, list) and filenames and len(set(filenames)) == len(filenames), "Invalid patch inventory")
        for filename in filenames:
            require(re.fullmatch(r"[0-9]{4}-[a-zA-Z0-9_.-]+\.patch", filename), "Invalid patch filename")
            require((patch_dir / filename).is_file() and not (patch_dir / filename).is_symlink(), "Missing or linked patch")
        require(set(filenames) == {path.name for path in patch_dir.glob("*.patch")}, "Unlisted archived patch")
        patches.append(patch)
        seen.add(name)
    return base, profile, patches, f"codefriendly-{base['tag']}-r{revision}"


def check_available(tag):
    # Require a successful metadata read and the fixed fork identity, not optional
    # user-token permissions flags. GitHub owner/repository names are case-insensitive.
    repo = api(f"/repos/{REPOSITORY}")
    require(isinstance(repo, dict) and isinstance(repo.get("full_name"), str), "Invalid fork repository response")
    name = repo["full_name"]
    require(name.isascii() and name.lower() == REPOSITORY,
            f"Fork repository identity mismatch: expected {REPOSITORY}, got {name!r}")
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


def verify_source(base, profile, patches):
    source = profile["source_commit"]
    # Fork checkouts need not contain upstream tags. Fetch only the canonical tag
    # into FETCH_HEAD, leaving any existing local tags untouched and untrusted.
    git("fetch", "--no-tags", f"https://github.com/{UPSTREAM}.git", f"refs/tags/{base['tag']}")
    require(git("rev-parse", "FETCH_HEAD^{commit}") == base["commit"], "Fetched upstream stable tag mismatch")
    require(git("rev-parse", f"{source}^{{commit}}") == source, "Source is not a commit")
    for tip in (source, "HEAD"):
        subprocess.run(["git", "merge-base", "--is-ancestor", base["commit"], tip], check=True)
    require(git("rev-parse", f"{source}^{{tree}}") == profile["source_tree"], "Source tree mismatch")
    cargo = tomllib.loads(git("show", f"{source}:Cargo.toml"))
    require(cargo["package"]["version"] == base["tag"][1:], "Source Cargo version differs from stable base")
    # Reconstruct only the tree in an isolated index; never execute feature scripts.
    # Unlike ancestry alone, this proves the ordered profile describes every source-tree change.
    with tempfile.TemporaryDirectory(prefix="fork-release-") as temp:
        env = dict(os.environ, GIT_INDEX_FILE=str(Path(temp) / "index"))
        git("read-tree", base["commit"], env=env)
        for patch in patches:
            patch_dir = Path("fork/patches") / patch["name"]
            for filename in patch["patches"]:
                git("apply", "--cached", str(patch_dir / filename), env=env)
        require(git("write-tree", env=env) == profile["source_tree"], "Archived profile does not reproduce the source tree")


def notes(base, profile, patches, workflow):
    inventory_lines = []
    for patch in patches:
        inventory_lines.append(f"### `{patch['name']}` from `{patch['source_branch']}` at `{patch['source_commit']}`")
        patch_dir = Path("fork/patches") / patch["name"]
        inventory_lines.extend(
            f"- `{patch['name']}/{filename}` — SHA-256 `{hashlib.sha256((patch_dir / filename).read_bytes()).hexdigest()}`"
            for filename in patch["patches"]
        )
    inventory = "\n".join(inventory_lines)
    return f"""Personal Codefriendly Herdr build; not an upstream release or endorsement.

## Provenance
- Upstream stable: `{base['tag']}` at `{base['commit']}` (herdrdev/herdr).
- Integrated source: `{profile['source_commit']}`; tree `{profile['source_tree']}`.
- Profile: `{profile['name']}`; source branch label: `{profile['source_branch']}` (not resolved at build time).
- Maintenance/workflow commit: `{workflow}` in codefriendly/herdr.
- Workflow: `.github/workflows/fork-release.yml`; run: https://github.com/{REPOSITORY}/actions/runs/{os.environ['GITHUB_RUN_ID']} (attempt {os.environ['GITHUB_RUN_ATTEMPT']}).

Ordered patch sets and archived patches from that maintenance commit:
{inventory}

## Validation scope
Automated for this source: published non-prerelease upstream tag and exact peeled base, base ancestry, recorded integration tree and Cargo version, and ordered profile reconstruction from archived patches. All five release-target builds and packaging jobs succeeded before publication; SHA256SUMS covers the five assets. Windows packaging includes the app-local ConPTY runtime and checks binary version identity.

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


def publish(base, profile, patches, tag, workflow):
    require(os.environ["RESOLVED_SHA"] == profile["source_commit"] and os.environ["RESOLVED_TAG"] == tag, "Resolved identity changed")
    assets = prepare_assets()
    body = notes(base, profile, patches, workflow)
    verify_upstream(base)
    check_available(tag)
    # POST ref is the atomic reservation: a racing tag creator gets a 422, never an update.
    # Any later failure leaves the tag/draft reserved. Retry only with a new revision.
    api(f"/repos/{REPOSITORY}/git/refs", method="POST", data={"ref": f"refs/tags/{tag}", "sha": profile["source_commit"]})
    release = api(f"/repos/{REPOSITORY}/releases", method="POST", data={
        "tag_name": tag, "target_commitish": profile["source_commit"],
        "name": f"Codefriendly Herdr {base['tag']} — revision {os.environ['REVISION']}",
        "body": body, "draft": True, "prerelease": False,
    })
    release_id = release["id"]
    require(type(release_id) is int and release["draft"] is True and release["tag_name"] == tag, "Unexpected release creation response")
    for name in (*ASSETS, "SHA256SUMS"):
        # Upload by newly created release ID, with no clobber/delete/replacement path.
        api(f"/repos/{REPOSITORY}/releases/{release_id}/assets?name={urllib.parse.quote(name)}", method="POST", data=(assets / name).read_bytes(), binary=True)
    ref = api(f"/repos/{REPOSITORY}/git/ref/tags/{tag}")
    require(ref["object"]["type"] == "commit" and ref["object"]["sha"] == profile["source_commit"], "Reserved tag changed during publication")
    api(f"/repos/{REPOSITORY}/releases/{release_id}", method="PATCH", data={"draft": False, "make_latest": "true"})


def main():
    require(len(sys.argv) == 2 and sys.argv[1] in ("resolve", "publish"), "Expected resolve or publish")
    workflow = dispatch_context()
    base, profile, patches, tag = metadata()
    if sys.argv[1] == "resolve":
        verify_upstream(base)
        verify_source(base, profile, patches)
        check_available(tag)
        with open(os.environ["GITHUB_OUTPUT"], "a") as output:
            output.write(f"commit={profile['source_commit']}\ntag={tag}\n")
    else:
        publish(base, profile, patches, tag, workflow)


if __name__ == "__main__":
    main()
