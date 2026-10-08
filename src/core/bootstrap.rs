use crate::{
    core::{Store, activation::Probe, source::AcquireOptions},
    models::{
        index::{Binding, Claimed},
        policy::Protection,
        source::Source,
        store::Request,
    },
};
use anyhow::{Result, ensure};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct Bootstrap {
    pub app: Uuid,
    pub claimed: Claimed,
    pub binding: Binding,
    pub shim: PathBuf,
}
pub fn saucepan_probe() -> Probe {
    Probe {
        args: vec!["--version".into()],
        stdout_contains: "saucepan ".into(),
        timeout_seconds: 10,
    }
}
impl Store {
    fn self_request(&self, request: &Request) -> Result<String> {
        ensure!(
            matches!(
                self.source(request.source_uuid)?.source,
                Source::GithubRelease { .. }
            ),
            "Saucepan self-management requires github_release"
        );
        ensure!(
            request.policies.protection == Some(Protection::UseableApp),
            "self-management requires useable_app"
        );
        ensure!(
            request.selection.bin.len() == 1,
            "self-management requires one declared executable"
        );
        Ok(request.selection.bin[0].clone())
    }
    pub fn bootstrap(
        &self,
        request: &Request,
        built_shim: &Path,
        options: &AcquireOptions,
    ) -> Result<Bootstrap> {
        let executable = self.self_request(request)?;
        let name = if cfg!(windows) {
            "saucepan.exe"
        } else {
            "saucepan"
        };
        ensure!(
            !self.root.join("bin").join(name).exists(),
            "already bootstrapped; use self-update"
        );
        let app = self.register_app("saucepan".into())?;
        let claimed = self.acquire_for(app.uuid, request, options)?;
        let binding = self.activate(
            app.uuid,
            claimed.claim.entry_uuid,
            &executable,
            &saucepan_probe(),
        )?;
        let shim = self.install_shim(app.uuid, name, built_shim, true)?;
        Ok(Bootstrap {
            app: app.uuid,
            claimed,
            binding,
            shim,
        })
    }
    pub fn self_update(
        &self,
        app: Uuid,
        claim: Uuid,
        options: &AcquireOptions,
    ) -> Result<Bootstrap> {
        let application = self.application(app)?;
        let request = &application
            .claims
            .get(&claim)
            .ok_or_else(|| anyhow::anyhow!("claim missing"))?
            .request;
        let executable = self.self_request(request)?;
        let claimed = self.update_claim(app, claim, options)?;
        let binding = self.activate(
            app,
            claimed.claim.entry_uuid,
            &executable,
            &saucepan_probe(),
        )?;
        Ok(Bootstrap {
            app,
            claimed,
            binding,
            shim: self.root.join("bin").join(if cfg!(windows) {
                "saucepan.exe"
            } else {
                "saucepan"
            }),
        })
    }
}
