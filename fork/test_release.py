"""Offline release guard tests; no GitHub calls or publication."""

import io
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

import release

ROOT = Path(__file__).resolve().parent.parent


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.cwd = Path.cwd()
        os.chdir(ROOT)
        self.addCleanup(os.chdir, self.cwd)
        self.source = "7a7023194477e003adbb7d8dc1a0b86095104257"
        self.env = patch.dict(os.environ, {
            "SOURCE_SHA": self.source, "REVISION": "1", "GH_TOKEN": "offline-test",
            "GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "1",
        })
        self.env.start()
        self.addCleanup(self.env.stop)

    def test_metadata_identity(self):
        base, archive, tag = release.metadata()
        self.assertEqual(base["tag"], "v0.9.0")
        self.assertEqual(archive["source_commit"], self.source)
        self.assertEqual(tag, "codefriendly-v0.9.0-r1")
        self.assertEqual(len(archive["patches"]), 3)

    def test_reject_nonexact_source_and_invalid_revision(self):
        for source in ("pane-hover-focus", self.source[:12], self.source.upper(), "a" * 40, "$(echo unsafe)"):
            with self.subTest(source=source), patch.dict(os.environ, SOURCE_SHA=source):
                with self.assertRaises(ValueError):
                    release.metadata()
        for revision in ("0", "-1", "01", "1.0", "1\n", "${{ github.token }}"):
            with self.subTest(revision=revision), patch.dict(os.environ, REVISION=revision):
                with self.assertRaises(ValueError):
                    release.metadata()

    def test_dispatch_requires_maintenance_commit_and_fork_master(self):
        sha = "b" * 40
        env = {"GITHUB_REPOSITORY": release.REPOSITORY, "GITHUB_EVENT_NAME": "workflow_dispatch",
               "GITHUB_REF": "refs/heads/master", "WORKFLOW_SHA": sha, "GITHUB_SHA": sha}
        with patch.dict(os.environ, env), patch.object(release, "git", return_value=sha):
            self.assertEqual(release.dispatch_context(), sha)
            for key, value in (("GITHUB_REPOSITORY", release.UPSTREAM), ("GITHUB_REF", "refs/heads/pane-hover-focus"),
                               ("GITHUB_EVENT_NAME", "push"), ("GITHUB_SHA", "c" * 40)):
                with self.subTest(key=key), patch.dict(os.environ, {key: value}):
                    with self.assertRaises(ValueError):
                        release.dispatch_context()

    def test_recorded_archive_reproduces_source(self):
        base, archive, _ = release.metadata()
        release.verify_source(base, archive)
        with patch.object(release, "git", wraps=release.git) as git:
            archive = dict(archive, source_tree="a" * 40)
            with self.assertRaisesRegex(ValueError, "Source tree mismatch"):
                release.verify_source(base, archive)
            self.assertFalse(any(call.args[0] == "read-tree" for call in git.call_args_list))

    def test_reordered_archive_fails(self):
        base, archive, _ = release.metadata()
        archive = dict(archive, patches=list(reversed(archive["patches"])))
        with self.assertRaises((ValueError, release.subprocess.CalledProcessError)):
            release.verify_source(base, archive)

    def test_upstream_published_stable_and_peeled_tag(self):
        base, _, _ = release.metadata()
        stable = {"tag_name": base["tag"], "draft": False, "prerelease": False, "published_at": "2026-01-01"}
        commit = {"type": "commit", "sha": base["commit"]}
        for annotated in (False, True):
            responses = [stable, {"object": {"type": "tag", "sha": "d" * 40} if annotated else commit}]
            if annotated:
                responses.append({"object": commit})
            with patch.object(release, "api", side_effect=responses):
                release.verify_upstream(base)
        for change in ({"draft": True}, {"prerelease": True}, {"published_at": None}, {"tag_name": "v0.9.1"}):
            with patch.object(release, "api", return_value=dict(stable, **change)):
                with self.assertRaises(ValueError):
                    release.verify_upstream(base)
        with patch.object(release, "api", side_effect=[stable, {"object": dict(commit, sha="e" * 40)}]):
            with self.assertRaisesRegex(ValueError, "peel"):
                release.verify_upstream(base)

    def test_collisions_include_tags_drafts_and_pagination(self):
        repo = {"full_name": release.REPOSITORY, "permissions": {"pull": True}}
        with patch.object(release, "api", side_effect=[repo, None, []]):
            release.check_available("candidate")
        for responses in ([repo, {"object": {}}], [repo, None, [{"tag_name": "candidate", "draft": True}]],
                          [repo, None, [{"tag_name": "other"}] * 100, [{"tag_name": "candidate"}]]):
            with patch.object(release, "api", side_effect=responses):
                with self.assertRaises(ValueError):
                    release.check_available("candidate")
        with patch.object(release, "api", side_effect=ValueError("API unavailable")):
            with self.assertRaises(ValueError):
                release.check_available("candidate")

    def test_api_only_accepts_explicit_get_404_as_absence(self):
        for status in (401, 403, 404, 429, 500):
            error = urllib.error.HTTPError("https://api.github.com/test", status, "failure", {}, io.BytesIO())
            with patch.object(release.urllib.request, "urlopen", side_effect=error):
                if status == 404:
                    self.assertIsNone(release.api("/test", missing=True))
                else:
                    with self.assertRaises(ValueError):
                        release.api("/test", missing=True)
                with self.assertRaises(ValueError):
                    release.api("/test", method="POST", data={}, missing=True)

    def test_notes_are_bounded_to_workflow_checks(self):
        base, archive, _ = release.metadata()
        body = release.notes(base, archive, "f" * 40)
        for text in (base["commit"], self.source, archive["source_tree"], "f" * 40, "does not run `just check`",
                     "Do not use the upstream Herdr updater", "does not attest to earlier local testing"):
            self.assertIn(text, body)
        for filename in archive["patches"]:
            self.assertIn(filename, body)
        self.assertNotIn("3312", body)

    def test_assets_exact_inventory_and_checksums(self):
        with tempfile.TemporaryDirectory() as temp:
            os.chdir(temp)
            Path("artifacts").mkdir()
            for name in release.ASSETS:
                (Path("artifacts") / name).write_bytes(name.encode())
            output = release.prepare_assets()
            self.assertEqual({p.name for p in output.iterdir()}, {*release.ASSETS, "SHA256SUMS"})
            self.assertEqual(len((output / "SHA256SUMS").read_text().splitlines()), 5)
        os.chdir(ROOT)
        with tempfile.TemporaryDirectory() as temp:
            os.chdir(temp)
            Path("artifacts").mkdir()
            (Path("artifacts") / "unexpected").write_text("no")
            with self.assertRaises(ValueError):
                release.prepare_assets()

    def test_publication_reserves_tag_then_creates_own_draft_without_overwrite(self):
        base, archive, tag = release.metadata()
        env = {"RESOLVED_SHA": self.source, "RESOLVED_TAG": tag}
        calls = []
        def fake_api(path, **kwargs):
            calls.append((path, kwargs))
            if path.endswith("/releases"):
                return {"id": 42, "draft": True, "tag_name": tag}
            if "/git/ref/tags/" in path:
                return {"object": {"type": "commit", "sha": self.source}}
            return {}
        with tempfile.TemporaryDirectory() as temp:
            assets = Path(temp)
            for name in (*release.ASSETS, "SHA256SUMS"):
                (assets / name).write_bytes(b"test")
            with patch.dict(os.environ, env), patch.object(release, "prepare_assets", return_value=assets), \
                    patch.object(release, "verify_upstream"), patch.object(release, "check_available") as available, \
                    patch.object(release, "api", side_effect=fake_api):
                release.publish(base, archive, tag, "f" * 40)
                available.assert_called_once_with(tag)
            self.assertTrue(calls[0][0].endswith("/git/refs"))
            self.assertEqual(calls[0][1]["method"], "POST")
            self.assertEqual(calls[1][1]["data"]["name"], "Codefriendly Herdr v0.9.0 — revision 1")
            uploads = [c for c in calls if "/assets?name=" in c[0]]
            self.assertEqual(len(uploads), 6)
            self.assertTrue(all(c[1]["method"] == "POST" and "/releases/42/" in c[0] for c in uploads))
            self.assertEqual(calls[-1][1], {"method": "PATCH", "data": {"draft": False, "make_latest": "true"}})
            with patch.dict(os.environ, env), patch.object(release, "prepare_assets", return_value=assets), \
                    patch.object(release, "verify_upstream"), patch.object(release, "check_available"), \
                    patch.object(release, "api", side_effect=ValueError("HTTP 422 collision")) as api:
                with self.assertRaisesRegex(ValueError, "collision"):
                    release.publish(base, archive, tag, "f" * 40)
                self.assertEqual(api.call_count, 1)


if __name__ == "__main__":
    unittest.main()
