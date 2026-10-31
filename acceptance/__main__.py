"""python -m acceptance [--tag TAG ...] [--name TEXT ...] [--list]"""
from __future__ import annotations

import argparse
import sys
import unittest

from . import REPO, report
from .sandbox import Run
from .scenario import Scenario, install_run


def scenarios() -> list[unittest.TestCase]:
    loader = unittest.TestLoader()
    suite = loader.discover(str(REPO / "acceptance" / "scenarios"), pattern="test_*.py",
                            top_level_dir=str(REPO))
    found: list[unittest.TestCase] = []

    def flatten(item):
        if isinstance(item, unittest.TestSuite):
            for child in item:
                flatten(child)
        elif isinstance(item, Scenario):
            found.append(item)
        elif isinstance(item, unittest.TestCase):  # import failures surface as cases
            found.append(item)

    flatten(suite)
    return sorted(found, key=lambda case: case.id())


def selected(case: unittest.TestCase, tags: list[str], names: list[str]) -> bool:
    if not tags and not names:
        return True
    case_tags = getattr(case, "tags", frozenset())
    return bool(set(tags) & set(case_tags)) or any(name in case.id() for name in names)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="python -m acceptance")
    parser.add_argument("--tag", action="append", default=[], help="run scenarios with this tag")
    parser.add_argument("--name", action="append", default=[], help="run scenarios whose id contains this text")
    parser.add_argument("--list", action="store_true", help="list scenarios and tags without running")
    args = parser.parse_args(argv)
    cases = scenarios()
    if args.list:
        for case in cases:
            print(f"{case.id()}  [{', '.join(sorted(getattr(case, 'tags', ())))}]")
        return 0
    run = Run()
    run.builds.manager()  # before isolation: cargo keeps the developer's environment
    run.isolate()
    install_run(run)
    outcomes = report.Outcomes()
    try:
        chosen = [case for case in cases if selected(case, args.tag, args.name)]
        unittest.TestSuite(chosen).run(outcomes)
    finally:
        run.restore()
    results = dict(outcomes.results)
    for case in cases:
        results.setdefault(case.id(), {"outcome": "not_selected"})
    written = report.write(run.dir, run.id, results, run.builds.built())
    print(f"report: {run.dir / 'report.json'}  {written['summary']}", file=sys.stderr)
    return 1 if written["summary"]["failed"] else 0


if __name__ == "__main__":
    sys.exit(main())
