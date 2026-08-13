from behave import given, then, when

from support import (
    make_git_repository,
    read_workspace_json,
    repository_commit,
    run_cli,
    write_bucket,
    write_workspace_config,
)


@given("a manifest-less repository indexed with an equivalent trailing-separator target")
def given_equivalent_target(context):
    context.repository = make_git_repository(context, "target")
    context.original_target = str(context.repository)
    context.bucket_file = context.test_root / "bucket.json"
    write_bucket(
        context.bucket_file,
        [
            {
                "name": "indexed-lib",
                "version": "1.0.0",
                "url": context.original_target + "/",
                "ref": repository_commit(context.repository),
                "manifest": {
                    "name": "indexed-lib",
                    "version": "1.0.0",
                    "description": "Indexed",
                },
            }
        ],
    )
    write_workspace_config(context, '[github]\nbinary = "git"\n')
    result = run_cli(context, "bucket", "add", str(context.bucket_file))
    assert result.returncode == 0, result.stderr


@when("I install the repository target without the trailing separator")
def when_install(context):
    run_cli(context, "install", context.original_target)


@then("installation succeeds with the index manifest identity")
def then_identity(context):
    assert context.result.returncode == 0, context.result.stderr
    assert read_workspace_json(context, ".saucepan/index.json")[0]["sauce"]["name"] == "indexed-lib"


@then("machine-readable state retains the original repository target")
def then_original_target(context):
    assert read_workspace_json(context, ".saucepan/index.json")[0]["repo"] == context.original_target
