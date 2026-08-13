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


def make_index(context):
    repository = make_git_repository(context, "index-repository")
    write_bucket(
        repository / "bucket.json",
        [{"name": "one", "version": "1.0.0", "url": "https://example.test/one"}],
    )
    context.initial_commit = commit_repository(repository, "add bucket")
    return repository


@given("a repository-target index tagged v1")
def given_tagged_index(context):
    context.index_repository = make_index(context)
    tag_repository(context.index_repository, "v1")
    write_workspace_config(context, '[local]\n[index]\nbinary = "git"\n')


@when("I register the index at ref v1")
def when_register_pinned(context):
    run_cli(
        context,
        "bucket",
        "add",
        str(context.index_repository),
        "--ref",
        "v1",
    )


@then("the registry records ref v1 and its resolved commit")
def then_pin_recorded(context):
    assert context.result.returncode == 0, context.result.stderr
    entry = read_workspace_json(context, ".saucepan/buckets.json")[0]
    assert entry["reference"] == "v1"
    assert entry["resolved_commit"] == context.initial_commit


@given("an unpinned registered repository-target index")
def given_unpinned_index(context):
    context.index_repository = make_index(context)
    write_workspace_config(context, '[local]\n[index]\nbinary = "git"\n')
    result = run_cli(context, "bucket", "add", str(context.index_repository))
    assert result.returncode == 0, result.stderr


@when("the index advances and I refresh it")
def when_advance_refresh(context):
    write_bucket(
        context.index_repository / "bucket.json",
        [
            {
                "name": "two",
                "version": "2.0.0",
                "url": "https://example.test/two",
            }
        ],
    )
    context.refreshed_commit = commit_repository(context.index_repository, "advance")
    run_cli(context, "bucket", "refresh", str(context.index_repository))


@then("the registry records the refreshed commit")
def then_refreshed(context):
    assert context.result.returncode == 0, context.result.stderr
    entry = read_workspace_json(context, ".saucepan/buckets.json")[0]
    assert entry["resolved_commit"] == context.refreshed_commit
