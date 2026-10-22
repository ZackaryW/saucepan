use super::*;
use crate::models::{
    index::lifecycle::Phase,
    policy::{
        PolicyDefinition, PolicyRecord,
        profile::{ProfileName, ProfileRequest},
    },
    setup::{Inputs, Outcome},
    source::Source,
    store::{AssetFilter, Format, Platform, Request, Selector},
};

pub(super) fn launcher_name() -> &'static str {
    if cfg!(windows) {
        "saucepan.exe"
    } else {
        "saucepan"
    }
}

pub(super) fn asset_name(tag: &str, platform: &Platform) -> Result<String> {
    let target = match (platform.os.as_str(), platform.arch.as_str()) {
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("windows", "x86") => "i686-pc-windows-msvc",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        _ => anyhow::bail!(
            "unsupported setup platform: {}/{}",
            platform.os,
            platform.arch
        ),
    };
    ensure!(
        !tag.is_empty()
            && tag
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c)),
        "invalid exact release tag"
    );
    Ok(format!("saucepan-{tag}-{target}.zip"))
}

pub(super) fn official_source() -> Source {
    Source::GithubRelease {
        repo: "ZackaryW/saucepan".into(),
    }
}

impl Store {
    pub(super) fn prepare_setup_record(&self, operation: Uuid, inputs: Inputs) -> Result<Record> {
        self.setup_admin()?;
        let _lock = self.lock()?;
        let profile = crate::core::policy::profile::expand(&ProfileRequest {
            name: ProfileName::ExplicitUpdate,
            for_install: true,
            ..Default::default()
        })?;
        let filter = PolicyRecord {
            uuid: Uuid::new_v4(),
            definition: PolicyDefinition::GithubReleaseAssetFilter(AssetFilter {
                platform: Some(inputs.platform.clone()),
                // Keep adopted selection stable when an explicit self-update changes the tag.
                // The exact release is selected by Request::selector, not by this platform filter.
                asset_patterns: vec![asset_name(&inputs.tag, &inputs.platform)?.replacen(
                    &format!("saucepan-{}-", inputs.tag),
                    "saucepan-*-",
                    1,
                )],
            }),
        };
        let source = super::super::source::git::normalize(official_source())?;
        let source_uuid = self
            .find_source(&source)?
            .map(|s| s.uuid)
            .unwrap_or_else(Uuid::new_v4);
        let request = Request {
            source_uuid,
            selector: Selector::Tag {
                tag: inputs.tag.clone(),
            },
            selection: crate::models::claim::request::Selection {
                platform: Some(inputs.platform.clone()),
                filter_refs: vec![filter.uuid].try_into()?,
                format: Format::Zip,
                bin: vec![launcher_name().into()],
                ..Default::default()
            },
            policy_refs: profile.combined_policy_refs.clone(),
        };
        let shim = self.setup_dir(operation)?.join("shim");
        let lifecycle_operation = Uuid::new_v4();
        let result = Outcome {
            schema_version: SCHEMA_VERSION,
            root: self.root.clone(),
            status: Phase::Preparing,
            changed: false,
            launcher_path: None,
            app: None,
            claim: None,
            output: None,
            installed_version: None,
            operation,
            lifecycle_operation: Some(lifecycle_operation),
            backup_paths: vec![],
            diagnostics: vec![],
            retry_shim: Some(shim.clone()),
        };
        Ok(Record {
            schema_version: SCHEMA_VERSION,
            operation,
            fingerprint: super::super::store::json_digest(&inputs)?,
            inputs,
            profile,
            filter,
            request,
            lifecycle_operation,
            shim,
            migration: None,
            result,
        })
    }
}
