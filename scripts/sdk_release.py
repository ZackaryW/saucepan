"""Exact SDK artifact validation shared by release preparation and publication."""
from email.parser import BytesParser
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import stat
import tarfile
import tomllib
import zipfile

MANIFEST = 'sdk-manifest.json'
MAX_ARCHIVE_BYTES = 32 * 1024 * 1024


def strict_json(data):
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError('duplicate JSON key')
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique)


def source_metadata(root=Path('.')):
    python = tomllib.loads((root/'sdk/python/pyproject.toml').read_text())['project']
    node = strict_json((root/'sdk/ts/package.json').read_text())
    for item in [python, node]:
        if item['name'] != 'saucepan-sdk' or not re.fullmatch(r'\d+\.\d+\.\d+(?:[-.][0-9A-Za-z.]+)?', item['version']):
            raise ValueError('invalid SDK package identity')
    return {
        'python': {'name': python['name'], 'version': python['version'],
                   'wheel': f"saucepan_sdk-{python['version']}-py3-none-any.whl", 'sdist': f"saucepan_sdk-{python['version']}.tar.gz"},
        'typescript': {'name': node['name'], 'version': node['version'], 'tarball': f"saucepan-sdk-{node['version']}.tgz"},
    }


def digest(path):
    with path.open('rb') as reader:
        return hashlib.file_digest(reader, 'sha256').hexdigest()


def safe_name(name):
    parts = name.rstrip('/').split('/')
    if not name or '\\' in name or ':' in name or any(part in ('', '.', '..') for part in parts) or PurePosixPath(name).is_absolute():
        raise ValueError('unsafe archive path')


def contents(path):
    """Read bounded regular members without extracting or trusting link entries."""
    if path.is_symlink() or not path.is_file() or path.stat().st_size > MAX_ARCHIVE_BYTES:
        raise ValueError('invalid SDK archive')
    result = {}
    total = 0
    if path.suffix == '.whl':
        with zipfile.ZipFile(path) as archive:
            for member in archive.infolist():
                safe_name(member.filename)
                mode = member.external_attr >> 16
                if member.is_dir() or member.filename in result or (stat.S_IFMT(mode) and not stat.S_ISREG(mode)):
                    raise ValueError('non-regular or duplicate wheel member')
                total += member.file_size
                if total > MAX_ARCHIVE_BYTES:
                    raise ValueError('SDK archive too large')
                result[member.filename] = archive.read(member)
    else:
        with tarfile.open(path, 'r:gz') as archive:
            seen = set()
            for member in archive:
                safe_name(member.name)
                if member.name in seen:
                    raise ValueError('duplicate tar member')
                seen.add(member.name)
                if member.isdir():
                    continue
                if not member.isfile():
                    raise ValueError('non-regular SDK member')
                total += member.size
                if total > MAX_ARCHIVE_BYTES:
                    raise ValueError('SDK archive too large')
                reader = archive.extractfile(member)
                if reader is None:
                    raise ValueError('unreadable SDK member')
                result[member.name] = reader.read()
    return result


def require_equal(files, name, expected):
    if files.get(name) != expected:
        raise ValueError(f'SDK source/content mismatch: {name}')


def validate_packages(directory, metadata, root=Path('.')):
    python = metadata['python']; node = metadata['typescript']
    wheel = contents(directory/python['wheel'])
    prefix = f"saucepan_sdk-{python['version']}"
    dist_info = prefix + '.dist-info/'
    meta = BytesParser().parsebytes(wheel[dist_info+'METADATA'])
    if meta.get_all('Name') != ['saucepan-sdk'] or meta.get_all('Version') != [python['version']]:
        raise ValueError('wheel identity differs from source')
    python_files = [path for path in (root/'sdk/python/src').rglob('*') if path.is_file() and (path.suffix == '.py' or path.name == 'py.typed')]
    expected_wheel = {str(path.relative_to(root/'sdk/python/src')).replace('\\','/') for path in python_files}
    expected_wheel.update({dist_info+'METADATA',dist_info+'WHEEL',dist_info+'RECORD',dist_info+'licenses/LICENSE','saucepan_sdk/README.md'})
    if set(wheel) != expected_wheel:
        raise ValueError('wheel member set differs from source package')
    for path in python_files:
        require_equal(wheel, path.relative_to(root/'sdk/python/src').as_posix(), path.read_bytes())
    require_equal(wheel, 'saucepan_sdk/README.md', (root/'sdk/python/README.md').read_bytes())
    require_equal(wheel, dist_info+'licenses/LICENSE', (root/'sdk/python/LICENSE').read_bytes())
    sdist = contents(directory/python['sdist'])
    for path in python_files:
        require_equal(sdist, prefix+'/src/'+path.relative_to(root/'sdk/python/src').as_posix(), path.read_bytes())
    for name in ['pyproject.toml','README.md','LICENSE']:
        require_equal(sdist, prefix+'/'+name, (root/'sdk/python'/name).read_bytes())
    for name in sdist:
        if not name.startswith(prefix+'/'):
            raise ValueError('unexpected sdist root')
    sdist_meta = BytesParser().parsebytes(sdist[prefix+'/PKG-INFO'])
    if sdist_meta.get_all('Name') != ['saucepan-sdk'] or sdist_meta.get_all('Version') != [python['version']]:
        raise ValueError('sdist identity differs from source')
    packed = contents(directory/node['tarball'])
    package = strict_json(packed['package/package.json'])
    if package != strict_json((root/'sdk/ts/package.json').read_text()):
        raise ValueError('npm metadata differs from source')
    expected_node = {'package/package.json','package/README.md','package/LICENSE'}
    for path in (root/'sdk/ts/src').rglob('*.ts'):
        stem = path.relative_to(root/'sdk/ts/src').with_suffix('').as_posix()
        expected_node.update({'package/dist/'+stem+'.js','package/dist/'+stem+'.d.ts'})
    if set(packed) != expected_node or any(not packed[name] for name in expected_node):
        raise ValueError('npm package missing complete compiled exports')
    for name in ['README.md','LICENSE']:
        require_equal(packed, 'package/'+name, (root/'sdk/ts'/name).read_bytes())


def expected_manifest(tag, commit, directory, root=Path('.')):
    if not re.fullmatch(r'[0-9a-f]{40}', commit):
        raise ValueError('expected exact release commit')
    metadata = source_metadata(root)
    validate_packages(directory, metadata, root)
    names = [metadata['python']['wheel'],metadata['python']['sdist'],metadata['typescript']['tarball']]
    return {'rust': {'tag':tag,'commit':commit}, **metadata, 'sha256': {name:digest(directory/name) for name in names}}


def write_manifest(tag, commit, directory, root=Path('.')):
    manifest = expected_manifest(tag,commit,directory,root)
    with (directory/MANIFEST).open('x',encoding='utf-8',newline='\n') as writer:
        writer.write(json.dumps(manifest,indent=2)+'\n')


def verify_manifest(tag, commit, directory, root=Path('.')):
    path=directory/MANIFEST
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 65536:
        raise ValueError('missing or invalid SDK manifest')
    manifest=strict_json(path.read_text())
    if manifest != expected_manifest(tag,commit,directory,root):
        raise ValueError('SDK manifest differs from release source/artifacts')
    return set(manifest['sha256']) | {MANIFEST}
