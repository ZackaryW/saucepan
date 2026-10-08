use crate::{
    core::{Store, activation::Probe, source::AcquireOptions},
    models::{
        index::lifecycle::{Installation, Operation},
        policy::Protection,
        source::Source,
        store::Request,
    },
};
use anyhow::{Result, ensure};
use std::path::Path;
use uuid::Uuid;

pub type Bootstrap = Operation;
pub fn saucepan_probe() -> Probe {
    Probe {
        args: vec!["--version".into()],
        stdout_contains: "saucepan ".into(),
        timeout_seconds: 10,
    }
}
impl Store {
    fn self_request(&self, request: &Request) -> Result<()> {
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
        Ok(())
    }
    pub fn bootstrap(
        &self,
        request: &Request,
        built_shim: &Path,
        options: &AcquireOptions,
    ) -> Result<Bootstrap> {
        self.bootstrap_with_operation(request, built_shim, options, None)
    }
    pub fn bootstrap_with_operation(
        &self,
        request: &Request,
        built_shim: &Path,
        options: &AcquireOptions,
        operation: Option<Uuid>,
    ) -> Result<Operation> {
        self.self_request(request)?;
        self.install_application(
            &Installation {
                name: "saucepan".into(),
                request: request.clone(),
                probe: saucepan_probe(),
                shim: if cfg!(windows) {
                    "saucepan.exe"
                } else {
                    "saucepan"
                }
                .into(),
                saucepan: true,
            },
            built_shim,
            options,
            operation,
        )
    }
    pub fn self_update(
        &self,
        app: Uuid,
        claim: Uuid,
        options: &AcquireOptions,
    ) -> Result<Bootstrap> {
        self.self_update_with_operation(app, claim, options, None)
    }
    pub fn self_update_with_operation(
        &self,
        app: Uuid,
        claim: Uuid,
        options: &AcquireOptions,
        operation: Option<Uuid>,
    ) -> Result<Operation> {
        // A terminal retry remains inspectable even after the application is removed.
        if let Some(id) = operation
            && self.operation_dir(id)?.join("index.json").exists()
        {
            return self.update_application(app, claim, &saucepan_probe(), None, options, Some(id));
        }
        let application = self.application(app)?;
        let request = &application
            .claims
            .get(&claim)
            .ok_or_else(|| anyhow::anyhow!("claim missing"))?
            .request;
        self.self_request(request)?;
        self.update_application(app, claim, &saucepan_probe(), None, options, operation)
    }
}
