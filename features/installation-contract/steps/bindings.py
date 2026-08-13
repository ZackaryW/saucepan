from behave import given, then, when

from support import make_git_repository, run_cli, write_workspace_config


@given("a manifest-less repository and no matching central index")
def given_manifestless_without_index(context):
    context.repository = make_git_repository(context, "target")
    write_workspace_config(context, '[github]\nbinary = "git"\n')


@when("I install the manifest-less repository")
def when_install(context):
    run_cli(context, "install", str(context.repository))


@then("the command exits with code 1")
def then_exit_one(context):
    assert context.result.returncode == 1, context.result.stderr


@then("stderr reports that no root manifest was found")
def then_missing_manifest(context):
    assert "no sauce.json found" in context.result.stderr
