from behave import given, then, when

from support import (
    make_git_repository,
    read_workspace_json,
    repository_commit,
    run_cli,
    write_bucket,
    write_workspace_config,
)


@given("a manifest-less repository and a registered index with a pinned manifest")
def given_indexed_target(context):
    context.repository = make_git_repository(context, "target")
    context.bucket_file = context.test_root / "bucket.json"
    write_bucket(
        context.bucket_file,
        [
            {
                "name": "indexed-lib",
                "version": "1.0.0",
                "url": str(context.repository),
                "ref": repository_commit(context.repository),
                "manifest": {
                    "name": "indexed-lib",
                    "version": "1.0.0",
                    "description": "Index supplied",
                },
            }
        ],
    )
    write_workspace_config(context, '[github]\nbinary = "git"\n')
    result = run_cli(context, "bucket", "add", str(context.bucket_file))
    assert result.returncode == 0, result.stderr


@when("I install the manifest-less repository")
def when_install(context):
    run_cli(context, "install", str(context.repository))


@then("installation succeeds with the index manifest identity")
def then_identity(context):
    assert context.result.returncode == 0, context.result.stderr
    entry = read_workspace_json(context, ".saucepan/index.json")[0]
    assert entry["sauce"]["name"] == "indexed-lib"


@then("machine-readable state records index manifest provenance")
def then_provenance(context):
    entry = read_workspace_json(context, ".saucepan/index.json")[0]
    assert entry["manifest_source"] == {
        "kind": "index",
        "index": str(context.bucket_file),
    }
