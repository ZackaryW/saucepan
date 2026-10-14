"""Build or verify independently installable SDK archives; never install Rust."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent
PYTHON = ROOT / 'sdk/python'
TS = ROOT / 'sdk/ts'


def run(args, **kwargs):
    subprocess.run([str(arg) for arg in args], check=True, **kwargs)


def npm(*args, cwd=TS):
    executable = shutil.which('npm.cmd' if os.name == 'nt' else 'npm')
    if not executable:
        raise RuntimeError('npm is required for package verification')
    run([executable, *args], cwd=cwd)


def build(destination):
    destination.mkdir(parents=True, exist_ok=True)
    run(['uv', 'build', PYTHON, '--out-dir', destination])
    npm('ci', '--ignore-scripts')
    npm('pack', '--pack-destination', destination)


def verify_python(artifact, base, binary, *, editable=False):
    environment = base / 'environment'
    run(['uv', 'venv', environment])
    executable = environment / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python')
    run(['uv', 'pip', 'install', '--python', executable, *(['--editable'] if editable else []), artifact])
    env = dict(os.environ, PATH='')
    env.pop('PYTHONPATH', None)
    code = ('from saucepan_sdk import Client; from saucepan_sdk.models.claim import EnsureRequest; '
            'Client().for_app("not-initialized"); '
            'print("standalone Python import without toolchain PATH")')
    run([executable, '-c', code], cwd=base, env=env)
    if binary:
        code = 'import sys; from saucepan_sdk import Client; c=Client(executable=sys.argv[1],root=sys.argv[2]); c.init(); assert c.query_sources()["sources"]==[]'
        run([executable, '-c', code, binary, base/'root'], cwd=base, env=env)


def verify(directory, binary, source_installs=False):
    version = tomllib.loads((PYTHON/'pyproject.toml').read_text())['project']['version']
    node_version = json.loads((TS/'package.json').read_text())['version']
    wheel = directory / f'saucepan_sdk-{version}-py3-none-any.whl'
    sdist = directory / f'saucepan_sdk-{version}.tar.gz'
    tarball = directory / f'saucepan-sdk-{node_version}.tgz'
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        assert 'saucepan_sdk/py.typed' in names
        assert 'saucepan_sdk/README.md' in names
        assert any(name.endswith('/LICENSE') for name in names)
    with tempfile.TemporaryDirectory(prefix='saucepan-installed-packages-') as temp:
        base = Path(temp)
        for name, artifact in [('wheel', wheel), ('sdist', sdist)]:
            directory_base = base/name; directory_base.mkdir()
            verify_python(str(artifact), directory_base, binary)
        if source_installs:
            for name, editable in [('local', False), ('editable', True)]:
                directory_base = base/name; directory_base.mkdir()
                verify_python(str(PYTHON), directory_base, binary, editable=editable)
            # Test an exact commit and subdirectory without publishing or changing
            # this repository's history. Copy only the installable Python source.
            repo = base/'git-fixture'; (repo/'sdk').mkdir(parents=True)
            shutil.copytree(PYTHON, repo/'sdk/python', ignore=shutil.ignore_patterns('.venv','dist','__pycache__','.mypy_cache','*.egg-info'))
            for args in [['init'], ['config','user.name','SDK fixture'], ['config','user.email','sdk@example.invalid'], ['add','.'], ['commit','-m','Source install fixture']]:
                run(['git','-C',repo,*args], stdout=subprocess.DEVNULL)
            commit = subprocess.check_output(['git','-C',str(repo),'rev-parse','HEAD'],text=True).strip()
            directory_base = base/'vcs'; directory_base.mkdir()
            verify_python(f'git+{repo.as_uri()}@{commit}#subdirectory=sdk/python', directory_base, binary)
        consumer = base/'node'; consumer.mkdir()
        (consumer/'package.json').write_text('{"type":"module","private":true}')
        npm('install','--ignore-scripts','--no-audit','--no-fund',tarball,cwd=consumer)
        env = dict(os.environ, PATH='')
        node = shutil.which('node')
        if not node:
            raise RuntimeError('Node is required for package verification')
        run([node,'--input-type=module','-e','import {Client} from "saucepan-sdk"; import {Client as C} from "saucepan-sdk/client"; new Client({executable:"absent"}); new C();'],cwd=consumer,env=env)
        if binary:
            run([node,'--input-type=module','-e','import {Client} from "saucepan-sdk"; const c=new Client({executable:process.argv[1],root:process.argv[2]}); await c.init(); if((await c.querySources()).sources.length)throw Error("unexpected sources");',binary,consumer/'root'],cwd=consumer,env=env)
        (consumer/'consumer.ts').write_text('''import {Client} from 'saucepan-sdk';
import type {EnsureRequest} from 'saucepan-sdk/models/claim';
const client=new Client();
const request:EnsureRequest={source_uuid:'source',selector:{type:'ref',reference:'HEAD'},scope:'project'};
void client.ensureClaim('app',request);
// @ts-expect-error unsupported selector discriminator
void client.ensureClaim('app',{...request,selector:{type:'invented'}});
// @ts-expect-error revisions are numbers
void client.removeGroup('group','not-a-revision');
''')
        run([node,TS/'node_modules/typescript/bin/tsc','--strict','--noEmit','--target','ES2022','--module','NodeNext','--moduleResolution','NodeNext','consumer.ts'],cwd=consumer)
    print('Installed wheel, sdist and npm package checks passed' + ('; local/editable/pinned Git source installs passed' if source_installs else ''))


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command',choices=['build','verify'])
    parser.add_argument('--directory',type=Path,required=True)
    parser.add_argument('--binary',type=Path)
    parser.add_argument('--source-installs',action='store_true')
    args=parser.parse_args()
    directory=args.directory.resolve()
    if args.command=='build': build(directory)
    else: verify(directory,args.binary.resolve() if args.binary else None,args.source_installs)


if __name__=='__main__': main()
