from behave import given, then, when


@given("two registered apps and a local source")
def registered(context):
    context.token = context.store.register("first")
    context.first = context.store.for_app(context.token)
    context.second = context.store.for_app(context.store.register("second"))
    source = context.test_root / "assets"
    source.mkdir()
    (source / "data.txt").write_text("data", encoding="utf-8")
    context.recipe = {"source": {"provider": "local", "path": str(source)}}


@when("the first app acquires the source through the Python SDK")
def acquire(context):
    context.result = context.first.acquire(context.recipe)


@then("only the first app sees the acquired entry")
def entries(context):
    assert set(context.first.view()["entries"]) == {context.result["artifact"]["id"]}
    assert context.second.view()["entries"] == {}


@then("its current view verifies through the Python SDK")
def verify(context):
    assert context.first.verify(context.first.view()) == {"verified": True}


@when("the first app changes its settings through the Python SDK")
def configure(context):
    context.settings = {"retain_snapshots": False, "verify_content": True, "allow_local_fallback": False}
    context.first.configure(settings=context.settings)


@then("its original caller token reads the new settings")
def stable_token(context):
    assert context.store.for_app(context.token).view()["settings"] == context.settings
    assert context.second.view()["settings"]["retain_snapshots"] is True
