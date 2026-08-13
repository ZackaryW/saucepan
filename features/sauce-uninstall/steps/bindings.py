import json

from behave import given, then, when

from support import make_git_repository, read_workspace_json, run_cli, write_workspace_config


@given("one managed sauce and one local sauce are installed")
def given_installed_sauces(context):
    context.managed_repository = make_git_repository(
        context,
        "managed-target",
        {
            "name": "managed-lib",
            "version": "1.0.0",
            "description": "Managed",
        },
    )
    write_workspace_config(context, '[github]\nbinary = "git"\n')
    installed = run_cli(context, "install", str(context.managed_repository))
    assert installed.returncode == 0, installed.stderr
    index = read_workspace_json(context, ".saucepan/index.json")
    context.managed_checkout = next(context.workspace_root.joinpath("github").iterdir())
    context.local_source = context.test_root / "local-source"
    context.local_source.mkdir()
    index.append(
        {
            "source_type": "local",
            "path": str(context.local_source),
            "sauce": {
                "name": "local-lib",
                "version": "1.0.0",
                "description": "Local",
            },
        }
    )
    (context.workspace_root / ".saucepan" / "index.json").write_text(
        json.dumps(index), encoding="utf-8"
    )


@when("I uninstall both sauces by manifest name")
def when_uninstall(context):
    context.managed_result = run_cli(context, "uninstall", "managed-lib")
    context.local_result = run_cli(context, "uninstall", "local-lib")


@then("both entries are absent from the local index")
def then_entries_absent(context):
    assert context.managed_result.returncode == 0, context.managed_result.stderr
    assert context.local_result.returncode == 0, context.local_result.stderr
    assert read_workspace_json(context, ".saucepan/index.json") == []


@then("the managed checkout is removed")
def then_managed_removed(context):
    assert not context.managed_checkout.exists()


@then("the local source path remains")
def then_local_remains(context):
    assert context.local_source.is_dir()
