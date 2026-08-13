from behave import given, then, when

from support import (
    commit_repository,
    make_git_repository,
    read_workspace_json,
    run_cli,
    tag_repository,
    write_bucket,
    write_workspace_config,
)


def manifest(version):
    return {
        "name": "indexed-lib",
        "version": version,
        "description": "Index supplied " + version,
    }


@given("an installed index-sourced sauce pinned by the index to v1")
def given_installed_v1(context):
    context.repository = make_git_repository(context, "target")
    context.v1_commit = tag_repository(context.repository, "v1")
    (context.repository / "README.md").write_text("v2\n", encoding="utf-8")
    context.v2_commit = commit_repository(context.repository, "v2 target")
    tag_repository(context.repository, "v2")
    context.bucket_file = context.test_root / "bucket.json"
    write_bucket(
        context.bucket_file,
        [
            {
                "name": "indexed-lib",
                "version": "1.0.0",
                "url": str(context.repository),
                "ref": "v1",
                "manifest": manifest("1.0.0"),
            }
        ],
    )
    write_workspace_config(context, '[github]\nbinary = "git"\n')
    assert run_cli(context, "bucket", "add", str(context.bucket_file)).returncode == 0
    installed = run_cli(context, "install", str(context.repository))
    assert installed.returncode == 0, installed.stderr


@given("the registered index now supplies manifest version 2.0.0 at ref v2")
def given_index_v2(context):
    write_bucket(
        context.bucket_file,
        [
            {
                "name": "indexed-lib",
                "version": "2.0.0",
                "url": str(context.repository),
                "ref": "v2",
                "manifest": manifest("2.0.0"),
            }
        ],
    )


@when("I update the installed sauce")
def when_update(context):
    run_cli(context, "update", "indexed-lib")


@then("the installed sauce reports version 2.0.0")
def then_version(context):
    assert context.result.returncode == 0, context.result.stderr
    context.updated_entry = read_workspace_json(context, ".saucepan/index.json")[0]
    assert context.updated_entry["sauce"]["version"] == "2.0.0"


@then("its resolved commit is the v2 commit")
def then_commit(context):
    assert context.updated_entry["resolved_commit"] == context.v2_commit


@then("its manifest provenance still identifies the registered index")
def then_provenance(context):
    assert context.updated_entry["manifest_source"] == {
        "kind": "index",
        "index": str(context.bucket_file),
    }
