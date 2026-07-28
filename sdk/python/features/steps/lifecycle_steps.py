import json
import os
import subprocess

from behave import given, then, when

from saucepan_sdk import Workspace


def commit_all(repository, message):
    environment = {
        **os.environ,
        "GIT_AUTHOR_NAME": "test",
        "GIT_AUTHOR_EMAIL": "test@example.com",
        "GIT_COMMITTER_NAME": "test",
        "GIT_COMMITTER_EMAIL": "test@example.com",
    }
    subprocess.run(
        ["git", "add", "sauce.json"],
        cwd=repository,
        env=environment,
        check=True,
        capture_output=True,
    )
    subprocess.run(
        ["git", "commit", "-m", message],
        cwd=repository,
        env=environment,
        check=True,
        capture_output=True,
    )


def write_manifest(repository, version):
    (repository / "sauce.json").write_text(
        json.dumps(
            {
                "name": "my-lib",
                "version": version,
                "description": "BDD sauce",
            }
        ),
        encoding="utf-8",
    )


def repository_commit(repository):
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repository,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    return result.stdout.strip()


@given('a GitHub sauce repository at version "{version}"')
def step_create_repository(context, version):
    source_root = context.test_root / "sources"
    context.repository = source_root / "repo.git"
    context.repository.mkdir(parents=True)
    subprocess.run(
        ["git", "init"], cwd=context.repository, check=True, capture_output=True
    )
    write_manifest(context.repository, version)
    commit_all(context.repository, "initial")
    context.expected_commit = repository_commit(context.repository)

    os.environ["GIT_CONFIG_COUNT"] = "1"
    os.environ["GIT_CONFIG_KEY_0"] = "url.{}/.insteadOf".format(
        source_root.as_uri()
    )
    os.environ["GIT_CONFIG_VALUE_0"] = "https://github.com/owner/"

    context.workspace_root = context.test_root / "workspace"
    context.workspace_root.mkdir()
    (context.workspace_root / "saucepan.toml").write_text(
        '[github]\nbinary = "git"\n', encoding="utf-8"
    )
    context.workspace = Workspace(
        context.workspace_root, binary=context.saucepan_binary
    )


@given('the repository commit is tagged "{reference}"')
def step_tag_repository(context, reference):
    subprocess.run(
        ["git", "tag", reference],
        cwd=context.repository,
        check=True,
        capture_output=True,
    )


@given("the repository is installed through the SDK")
def step_install_repository(context):
    context.sauce = context.workspace.install("owner/repo")


@given('the repository advances to version "{version}"')
def step_advance_repository(context, version):
    write_manifest(context.repository, version)
    commit_all(context.repository, "advance")
    context.expected_commit = repository_commit(context.repository)


@when("I install the repository without a ref")
def step_install_without_ref(context):
    context.sauce = context.workspace.install("owner/repo")


@when('I install the repository with ref "{reference}"')
def step_install_with_ref(context, reference):
    context.sauce = context.workspace.install("owner/repo", reference=reference)


@when("I update the installed sauce")
def step_update_sauce(context):
    context.updated_sauce = context.sauce.update()


@when("I uninstall the installed sauce")
def step_uninstall_sauce(context):
    context.sauce.uninstall()


@then('the returned sauce is named "{name}"')
def step_assert_name(context, name):
    assert context.sauce.name == name


@then("the returned sauce has no requested ref")
def step_assert_no_reference(context):
    assert context.sauce.reference is None


@then('the returned sauce requested ref is "{reference}"')
def step_assert_reference(context, reference):
    assert context.sauce.reference == reference


@then("the returned sauce records the repository commit")
def step_assert_commit(context):
    assert context.sauce.resolved_commit == context.expected_commit


@then('the same sauce reports version "{version}"')
def step_assert_updated_version(context, version):
    assert context.updated_sauce is context.sauce
    assert context.sauce.version == version


@then("the workspace has no installed sauces")
def step_assert_no_sauces(context):
    assert context.workspace.sauces == ()
