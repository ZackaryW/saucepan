import json

from behave import given, then, when

from support import (
    make_git_repository,
    repository_commit,
    run_cli,
    write_bucket,
)


@given("two registered indexes describe the same manifest-less target")
def given_colliding_indexes(context):
    context.repository = make_git_repository(context, "target")
    target_ref = repository_commit(context.repository)
    context.early_index = context.test_root / "early.json"
    context.late_index = context.test_root / "late.json"
    for path, name in (
        (context.early_index, "early-lib"),
        (context.late_index, "late-lib"),
    ):
        write_bucket(
            path,
            [
                {
                    "name": name,
                    "version": "1.0.0",
                    "url": str(context.repository),
                    "ref": target_ref,
                    "manifest": {
                        "name": name,
                        "version": "1.0.0",
                        "description": name,
                    },
                }
            ],
        )


@when("I install the target twice in separate workspaces")
def when_install_twice(context):
    context.install_results = []
    context.install_workspaces = []
    for suffix in ("one", "two"):
        workspace = context.test_root / ("workspace-" + suffix)
        workspace.mkdir()
        (workspace / "saucepan.toml").write_text(
            '[github]\nbinary = "git"\n', encoding="utf-8"
        )
        for index in (context.early_index, context.late_index):
            registered = run_cli(context, "bucket", "add", str(index), workspace=workspace)
            assert registered.returncode == 0, registered.stderr
        result = run_cli(context, "install", str(context.repository), workspace=workspace)
        context.install_results.append(result)
        context.install_workspaces.append(workspace)


@then("each resolution warns that the later index is shadowed")
def then_each_warns(context):
    assert all(result.returncode == 0 for result in context.install_results)
    for result in context.install_results:
        assert "shadowed by" in result.stderr
        assert str(context.late_index) in result.stderr
        assert str(context.early_index) in result.stderr


@then("each installation uses the earlier index manifest")
def then_earlier_wins(context):
    for workspace in context.install_workspaces:
        index = json.loads(
            (workspace / ".saucepan" / "index.json").read_text(encoding="utf-8")
        )
        assert index[0]["sauce"]["name"] == "early-lib"
