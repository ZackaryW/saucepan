"""Reject incomplete or inconsistent SDK distributions without invoking builders."""
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

import sdk_release


def tar(path, members):
    with tarfile.open(path, 'w:gz') as archive:
        for name, data in members.items():
            member=tarfile.TarInfo(name); member.size=len(data)
            archive.addfile(member,io.BytesIO(data))


def sdk_assets(directory, tag, commit):
    directory.mkdir(parents=True,exist_ok=True)
    meta=sdk_release.source_metadata()
    py=meta['python']; node=meta['typescript']
    prefix=f"saucepan_sdk-{py['version']}"
    metadata=f"Metadata-Version: 2.4\nName: saucepan-sdk\nVersion: {py['version']}\n\n".encode()
    wheel={prefix+'.dist-info/METADATA':metadata,prefix+'.dist-info/WHEEL':b'Wheel-Version: 1.0\n',prefix+'.dist-info/RECORD':b'',
           prefix+'.dist-info/licenses/LICENSE':Path('sdk/python/LICENSE').read_bytes(),'saucepan_sdk/README.md':Path('sdk/python/README.md').read_bytes()}
    sdist={prefix+'/PKG-INFO':metadata}
    for path in Path('sdk/python/src').rglob('*'):
        if path.is_file() and (path.suffix=='.py' or path.name=='py.typed'):
            relative=path.relative_to('sdk/python/src').as_posix()
            wheel[relative]=path.read_bytes(); sdist[prefix+'/src/'+relative]=path.read_bytes()
    for name in ['README.md','LICENSE','pyproject.toml']:
        sdist[prefix+'/'+name]=Path('sdk/python',name).read_bytes()
    with zipfile.ZipFile(directory/py['wheel'],'w') as archive:
        for name,data in wheel.items(): archive.writestr(name,data)
    tar(directory/py['sdist'],sdist)
    package={f'package/{name}':Path('sdk/ts',name).read_bytes() for name in ['package.json','README.md','LICENSE']}
    for path in Path('sdk/ts/src').rglob('*.ts'):
        stem=path.relative_to('sdk/ts/src').with_suffix('').as_posix()
        for suffix in ['.js','.d.ts']: package['package/dist/'+stem+suffix]=b'export {};\n'
    tar(directory/node['tarball'],package)
    sdk_release.write_manifest(tag,commit,directory)
    return meta


class SdkRelease(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.directory=Path(self.temp.name)
        self.tag='v0.1.0';self.commit='a'*40
        self.meta=sdk_assets(self.directory,self.tag,self.commit)

    def test_validated_exact_artifacts(self):
        names=sdk_release.verify_manifest(self.tag,self.commit,self.directory)
        self.assertEqual(len(names),4)

    def test_other_commit_tag_or_duplicate_manifest_key(self):
        for tag,commit in [(self.tag,'b'*40),('v9.0.0',self.commit)]:
            with self.assertRaises(ValueError): sdk_release.verify_manifest(tag,commit,self.directory)
        path=self.directory/sdk_release.MANIFEST
        path.write_text('{"rust":{},"rust":{}}')
        with self.assertRaises(ValueError): sdk_release.verify_manifest(self.tag,self.commit,self.directory)

    def test_missing_or_changed_package_is_rejected(self):
        path=self.directory/self.meta['python']['wheel']
        with zipfile.ZipFile(path,'a') as archive:
            archive.writestr('saucepan_sdk/unexpected.py',b'bad')
        with self.assertRaises(ValueError): sdk_release.verify_manifest(self.tag,self.commit,self.directory)
        path.unlink()
        with self.assertRaises(ValueError): sdk_release.verify_manifest(self.tag,self.commit,self.directory)

    def test_wrong_package_metadata(self):
        path=self.directory/self.meta['typescript']['tarball']
        members=sdk_release.contents(path)
        package=json.loads(members['package/package.json']); package['version']='9.9.9'
        members['package/package.json']=json.dumps(package).encode();tar(path,members)
        with self.assertRaises(ValueError): sdk_release.verify_manifest(self.tag,self.commit,self.directory)

    def test_unsafe_duplicate_and_link_members(self):
        path=self.directory/'unsafe.tgz'
        for name in ['../escape','/absolute','package/../escape','package\\escape','C:/escape']:
            tar(path,{name:b'bad'})
            with self.assertRaises(ValueError): sdk_release.contents(path)
        with tarfile.open(path,'w:gz') as archive:
            member=tarfile.TarInfo('package/file');member.type=tarfile.SYMTYPE;member.linkname='../outside';archive.addfile(member)
        with self.assertRaises(ValueError): sdk_release.contents(path)
        with tarfile.open(path,'w:gz') as archive:
            for _ in range(2):
                member=tarfile.TarInfo('package/file');member.size=1;archive.addfile(member,io.BytesIO(b'x'))
        with self.assertRaises(ValueError): sdk_release.contents(path)

    def test_changed_source_and_manifest_hash_rejected(self):
        path=self.directory/sdk_release.MANIFEST
        data=json.loads(path.read_text());data['sha256'][self.meta['python']['wheel']]='0'*64
        path.write_text(json.dumps(data))
        with self.assertRaises(ValueError): sdk_release.verify_manifest(self.tag,self.commit,self.directory)
