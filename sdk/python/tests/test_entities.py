import json
from pathlib import Path
from unittest import mock

from saucepan_sdk import Bucket, BucketStub, Sauce, Workspace


def test_sauces_are_snapshots_from_the_full_index(
    saucepan_binary: Path, workspace: Path
) -> None:
    write_index(
        workspace,
        [
            {
                "source_type": "github",
                "repo": "owner/with-revision",
                "reference": "main",
                "resolved_commit": "abc123",
                "sauce": {
                    "name": "with-revision",
                    "version": "2.0.0",
                    "description": "Tracks a branch",
                },
            },
            {
                "source_type": "github",
                "repo": "owner/legacy",
                "sauce": {
                    "name": "legacy",
                    "version": "1.0.0",
                    "description": "No revision fields",
                },
            },
        ],
    )
    client = Workspace(workspace, binary=saucepan_binary)

    sauces = client.sauces

    assert all(isinstance(sauce, Sauce) for sauce in sauces)
    assert [
        (sauce.name, sauce.version, sauce.description) for sauce in sauces
    ] == [
        ("with-revision", "2.0.0", "Tracks a branch"),
        ("legacy", "1.0.0", "No revision fields"),
    ]
    assert sauces[0].reference == "main"
    assert sauces[0].resolved_commit == "abc123"
    assert sauces[1].reference is None
    assert sauces[1].resolved_commit is None


def test_buckets_are_entities_from_the_registry(
    saucepan_binary: Path, workspace: Path
) -> None:
    state = workspace / ".saucepan"
    state.mkdir()
    (state / "buckets.json").write_text(
        json.dumps([{"url": "https://example.test/bucket.json"}]),
        encoding="utf-8",
    )
    client = Workspace(workspace, binary=saucepan_binary)

    buckets = client.buckets

    assert len(buckets) == 1
    assert isinstance(buckets[0], Bucket)
    assert buckets[0].url == "https://example.test/bucket.json"


def test_sauce_reports_repository_manifest_source() -> None:
    entry = {
        "manifest_source": {"kind": "repository"},
        "sauce": {
            "name": "my-lib",
            "version": "1.0.0",
            "description": "Test sauce",
        },
    }

    sauce = Sauce(mock.Mock(), entry)

    assert sauce.manifest_source == {"kind": "repository"}


def test_sauce_reports_index_manifest_source() -> None:
    entry = {
        "manifest_source": {"kind": "index", "index": "owner/central-index"},
        "sauce": {
            "name": "my-lib",
            "version": "1.0.0",
            "description": "Test sauce",
        },
    }

    sauce = Sauce(mock.Mock(), entry)

    assert sauce.manifest_source == {
        "kind": "index",
        "index": "owner/central-index",
    }


def test_sauce_defaults_manifest_source_to_repository_when_absent() -> None:
    entry = {
        "sauce": {
            "name": "my-lib",
            "version": "1.0.0",
            "description": "Test sauce",
        },
    }

    sauce = Sauce(mock.Mock(), entry)

    assert sauce.manifest_source == {"kind": "repository"}


def test_bucket_stub_exposes_extra_fields_alongside_required_ones() -> None:
    entry = {
        "name": "my-lib",
        "version": "1.0.0",
        "url": "https://example.test/my-lib/sauce.json",
        "manifest": {"commands": ["build", "test"]},
        "note": "curated by zush",
    }

    stub = BucketStub(entry)

    assert stub.name == "my-lib"
    assert stub.version == "1.0.0"
    assert stub.url == "https://example.test/my-lib/sauce.json"
    assert stub.extra == {
        "manifest": {"commands": ["build", "test"]},
        "note": "curated by zush",
    }
    assert stub == entry


def test_bucket_stub_defaults_extra_to_empty_mapping_when_absent() -> None:
    entry = {
        "name": "other-lib",
        "version": "2.0.0",
        "url": "https://example.test/other-lib/sauce.json",
    }

    stub = BucketStub(entry)

    assert stub.extra == {}
    assert stub == entry


def test_bucket_stubs_wraps_each_parsed_entry_as_a_bucket_stub() -> None:
    raw_entries = [
        {
            "name": "my-lib",
            "version": "1.0.0",
            "url": "https://example.test/my-lib/sauce.json",
            "note": "curated by zush",
        },
        {
            "name": "other-lib",
            "version": "2.0.0",
            "url": "https://example.test/other-lib/sauce.json",
        },
    ]
    workspace_double = mock.Mock()
    workspace_double.cat_bucket.return_value = raw_entries
    bucket = Bucket(workspace_double, "https://example.test/bucket.json")

    stubs = bucket.stubs()

    assert all(isinstance(stub, BucketStub) for stub in stubs)
    assert stubs == raw_entries
    assert stubs[0].extra == {"note": "curated by zush"}
    assert stubs[1].extra == {}


def write_index(workspace: Path, entries: object) -> None:
    state = workspace / ".saucepan"
    state.mkdir(exist_ok=True)
    (state / "index.json").write_text(json.dumps(entries), encoding="utf-8")
