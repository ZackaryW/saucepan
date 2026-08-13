from behave import given, then, when

from support import run_cli, write_bucket, write_workspace_config


@given("a registered local bucket containing my-lib")
def given_bucket(context):
    write_workspace_config(context, "[local]\n")
    context.bucket_file = context.test_root / "bucket.json"
    write_bucket(
        context.bucket_file,
        [
            {
                "name": "my-lib",
                "version": "1.0.0",
                "url": "https://example.test/my-lib",
            }
        ],
    )
    result = run_cli(context, "bucket", "add", str(context.bucket_file))
    assert result.returncode == 0, result.stderr


@when("I search for the entry named my-lib")
def when_search(context):
    run_cli(context, "search", '.name == "my-lib"')


@then("stdout contains the my-lib entry")
def then_match(context):
    assert context.result.returncode == 0, context.result.stderr
    assert '"name":"my-lib"' in context.result.stdout.replace(" ", "")
