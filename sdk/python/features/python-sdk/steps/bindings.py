from behave import given, then, when

from saucepan_sdk import Workspace
from support import advance_index, make_index_repository, write_sdk_state_fixture


@given("an SDK workspace and a repository-target index tagged v1")
def given_sdk_tagged_index(context):
    context.index_repository = make_index_repository(context, "v1")
    context.workspace = Workspace(context.workspace_root, binary=context.saucepan_binary)


@when("I add the index through the SDK at ref v1")
def when_sdk_add_pinned(context):
    context.bucket = context.workspace.add_bucket(
        str(context.index_repository), reference="v1"
    )


@then("the SDK bucket collection records ref v1 and its resolved commit")
def then_sdk_pin(context):
    entry = context.workspace.cat_buckets()[0]
    assert entry["reference"] == "v1"
    assert entry["resolved_commit"] == context.index_commit
    assert context.bucket.url == str(context.index_repository)


@given("an SDK workspace with an unpinned registered repository-target index")
def given_sdk_unpinned_index(context):
    context.index_repository = make_index_repository(context, "v1")
    context.workspace = Workspace(context.workspace_root, binary=context.saucepan_binary)
    context.workspace.add_bucket(str(context.index_repository))


@when("the index advances and I refresh it through the SDK")
def when_sdk_refresh(context):
    context.refreshed_commit = advance_index(context.index_repository)
    context.bucket = context.workspace.refresh_bucket(str(context.index_repository))


@then("the SDK bucket collection records the refreshed commit")
def then_sdk_refresh(context):
    entry = context.workspace.cat_buckets()[0]
    assert entry["resolved_commit"] == context.refreshed_commit
    assert context.bucket.url == str(context.index_repository)


@given("CLI state containing an index-sourced sauce and a bucket stub with extra fields")
def given_sdk_state(context):
    write_sdk_state_fixture(context)
    context.workspace = Workspace(context.workspace_root, binary=context.saucepan_binary)


@when("I read sauces and bucket stubs through the SDK")
def when_read_entities(context):
    context.sauce = context.workspace.sauces[0]
    context.stub = context.workspace.buckets[0].stubs()[0]


@then("the sauce exposes its index manifest source")
def then_manifest_source(context):
    assert context.sauce.manifest_source["kind"] == "index"


@then("the bucket stub exposes its extra fields")
def then_stub_extra(context):
    assert context.stub.extra == {"commands": {"run": "bin/run"}}
