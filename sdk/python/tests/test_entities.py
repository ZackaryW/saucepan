import json
from pathlib import Path

from saucepan_sdk import Bucket, Sauce, Workspace


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


def write_index(workspace: Path, entries: object) -> None:
    state = workspace / ".saucepan"
    state.mkdir(exist_ok=True)
    (state / "index.json").write_text(json.dumps(entries), encoding="utf-8")
