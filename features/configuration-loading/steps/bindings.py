from behave import given, then, when

from support import run_cli


@given("a workspace without saucepan.toml")
def given_missing_config(context):
    assert not (context.workspace_root / "saucepan.toml").exists()


@when("I run the list command")
def when_list(context):
    run_cli(context, "list")


@then("the command exits with code 3")
def then_exit_three(context):
    assert context.result.returncode == 3, context.result.stderr


@then("stderr names saucepan.toml")
def then_stderr_names_config(context):
    assert "saucepan.toml" in context.result.stderr
