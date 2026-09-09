use super::{
    crypto::Keys,
    keyring::{NativeKeyring, SecretStore},
    layout::Layout,
    locking::Lock,
    transactions::Generations,
};
use crate::{core::models::*, utils};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct Enrollment {
    schema_version: u32,
    store_id: String,
    key_generation: u64,
    master: [u8; 32],
}

/// A session never exposes secrets, raw authority records, or storage mutation.
pub(in crate::core) struct Session {
    layout: Layout,
    enrollment: Enrollment,
    keys: Keys,
}

mod access;
mod dependencies;
mod installations;
mod materializations;
mod mirrors;
mod pins;

impl Session {
    #[cfg(any(test, feature = "test-support"))]
    pub(in crate::core) fn initialize_test(root: &Path, key: [u8; 32]) -> Result<Self> {
        let layout = Layout::at_test_root(root)?;
        let secrets = TestSecrets {
            layout: layout.clone(),
            key: Zeroizing::new(key),
        };
        Self::initialize_with_master(layout, &secrets, Some(Zeroizing::new(key)))
    }
    #[cfg(any(test, feature = "test-support"))]
    pub(in crate::core) fn open_test(root: &Path, key: [u8; 32]) -> Result<Self> {
        let layout = Layout::at_test_root(root)?;
        let secrets = TestSecrets {
            layout: layout.clone(),
            key: Zeroizing::new(key),
        };
        Self::open(layout, &secrets)
    }
    pub(in crate::core) fn open_user() -> Result<Self> {
        let layout = Layout::for_user()?;
        let keyring = Self::keyring(&layout)?;
        Self::open(layout, &keyring)
    }
    pub(in crate::core) fn initialize_user() -> Result<Self> {
        let layout = Layout::for_user()?;
        let keyring = Self::keyring(&layout)?;
        Self::initialize(layout, &keyring)
    }
    fn keyring(layout: &Layout) -> Result<NativeKeyring> {
        let bytes = layout.root().as_os_str().as_encoded_bytes();
        let account = utils::hex(&Sha256::digest(utils::frame(
            b"saucepan/keyring-account/v1",
            &[bytes],
        )));
        NativeKeyring::new("org.saucepan.store.v1", &account)
    }
    fn initialize(layout: Layout, secrets: &dyn SecretStore) -> Result<Self> {
        Self::initialize_with_master(layout, secrets, None)
    }
    fn initialize_with_master(
        layout: Layout,
        secrets: &dyn SecretStore,
        supplied: Option<Zeroizing<[u8; 32]>>,
    ) -> Result<Self> {
        if secrets.read()?.is_some() {
            return Err(Error::new(
                ErrorKind::Conflict,
                "keyring enrollment already exists; use explicit recovery",
            ));
        }
        // create_dir wins exclusive initialization. Any subsequent partial state
        // is explicit recovery, never an empty-authority fallback.
        layout.initialize_root()?;
        let mut random_id = [0; 16];
        let mut master = Zeroizing::new([0; 32]);
        getrandom::fill(&mut random_id)
            .and_then(|_| getrandom::fill(master.as_mut()))
            .map_err(|_| Error::new(ErrorKind::Internal, "OS random source unavailable"))?;
        if let Some(supplied) = supplied {
            master.copy_from_slice(supplied.as_ref());
        }
        let enrollment = Enrollment {
            schema_version: 1,
            store_id: utils::hex(&random_id),
            key_generation: 1,
            master: *master,
        };
        let blob =
            Zeroizing::new(serde_json::to_vec(&enrollment).map_err(|_| {
                Error::new(ErrorKind::Internal, "cannot encode keyring enrollment")
            })?);
        secrets.write(&blob)?;
        let keys = Keys::derive(&enrollment.master, &enrollment.store_id)?;
        let session = Self {
            layout,
            enrollment,
            keys,
        };
        let mut index = CentralIndex::empty(session.enrollment.store_id.clone());
        let owner_root = session
            .layout
            .root()
            .to_str()
            .ok_or_else(|| Error::new(ErrorKind::Config, "store path is not UTF-8"))?
            .to_owned();
        let owner = Registration {
            id: "owner".into(),
            root: owner_root.clone(),
            revision: 1,
            data_generation: 0,
            mode: ApplicationMode::Authoritative,
            revoked: false,
            require_verification: false,
            grants: vec![Grant {
                source_id: None,
                selection: ".".into(),
                actions: [Action::Manage].into_iter().collect(),
                destinations: vec![],
            }],
        };
        index.applications.insert(owner.id.clone(), owner);
        let mut nonce = [0; 32];
        getrandom::fill(&mut nonce)
            .map_err(|_| Error::new(ErrorKind::Internal, "OS random source unavailable"))?;
        let marker = session.keys.sign(MarkerClaims {
            schema_version: 1,
            store_id: index.store_id.clone(),
            registration_id: "owner".into(),
            registration_revision: 1,
            enrolled_root: owner_root,
            nonce: utils::hex(&nonce),
            key_generation: 1,
        })?;
        // Publish the credential first: a crash before index publication is
        // recognizable partial initialization, never an empty usable store.
        session.layout.replace(
            Path::new("owner.saucepanhash"),
            &serde_json::to_vec(&marker)
                .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode owner credential"))?,
        )?;
        session.generations().initialize(&index)?;
        Ok(session)
    }
    fn open(layout: Layout, secrets: &dyn SecretStore) -> Result<Self> {
        let blob = secrets.read()?.ok_or_else(|| {
            Error::new(
                ErrorKind::Authority,
                "store key is unavailable; initialize explicitly or recover the established store",
            )
        })?;
        if blob.len() > 4096 {
            return Err(Error::new(
                ErrorKind::Integrity,
                "invalid keyring enrollment",
            ));
        }
        let enrollment: Enrollment = serde_json::from_slice(&blob)
            .map_err(|_| Error::new(ErrorKind::Integrity, "invalid keyring enrollment"))?;
        if enrollment.schema_version != 1 || enrollment.key_generation == 0 {
            return Err(Error::new(
                ErrorKind::Compatibility,
                "unsupported keyring enrollment version",
            ));
        }
        if enrollment.store_id.len() != 32
            || !enrollment.store_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error::new(
                ErrorKind::Integrity,
                "invalid keyring store identity",
            ));
        }
        let keys = Keys::derive(&enrollment.master, &enrollment.store_id)?;
        let session = Self {
            layout,
            enrollment,
            keys,
        };
        session.read_index()?;
        Ok(session)
    }
    fn generations(&self) -> Generations<'_> {
        Generations {
            layout: &self.layout,
            keys: &self.keys,
            store_id: &self.enrollment.store_id,
            key_generation: self.enrollment.key_generation,
        }
    }
    pub(in crate::core) fn read_index(&self) -> Result<CentralIndex> {
        let _lock = Lock::acquire(
            &self.layout,
            "central",
            Instant::now() + Duration::from_secs(5),
        )?;
        self.generations().recover()?;
        Ok(self.generations().read()?.central)
    }
    pub(in crate::core) fn inspect_context(
        &self,
        root: &Path,
        marker: Option<&Marker>,
    ) -> Result<Registration> {
        let index = self.read_index()?;
        self.context_in(&index, root, marker)
    }
    fn context_in(
        &self,
        index: &CentralIndex,
        root: &Path,
        marker: Option<&Marker>,
    ) -> Result<Registration> {
        let root = std::fs::canonicalize(root).map_err(|_| Error::not_found())?;
        let root = root
            .to_str()
            .ok_or_else(|| Error::new(ErrorKind::Config, "application path is not UTF-8"))?;
        let registration = index
            .applications
            .values()
            .find(|r| r.root == root && !r.revoked)
            .ok_or_else(Error::not_found)?;
        if registration.mode == ApplicationMode::Authoritative || marker.is_some() {
            let marker = marker.ok_or_else(|| {
                Error::new(
                    ErrorKind::Authority,
                    "application registration credential required",
                )
            })?;
            self.keys.verify(marker)?;
            let c = &marker.claims;
            if c.store_id != index.store_id
                || c.registration_id != registration.id
                || c.registration_revision != registration.revision
                || c.enrolled_root != root
                || c.key_generation != index.key_generation
            {
                return Err(Error::new(
                    ErrorKind::Authority,
                    "application registration is not current",
                ));
            }
        }
        Ok(registration.clone())
    }
    pub(in crate::core) fn check_store(&self) -> Result<()> {
        // A consistency probe returns no global metadata or grants.
        self.read_index().map(|_| ())
    }
    pub(in crate::core) fn set_source_policy(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        id: &str,
        mut policy: CachePolicy,
    ) -> Result<()> {
        let context = self.inspect_context(root, marker)?;
        require_source_management(&context, id)?;
        let _locks = Lock::transaction(
            &self.layout,
            [id.to_owned()],
            [],
            Instant::now() + Duration::from_secs(5),
        )?;
        self.generations().recover()?;
        let mut snapshot = self.generations().read()?;
        let context = self.context_in(&snapshot.central, root, marker)?;
        require_source_management(&context, id)?;
        let mut source = snapshot.sources.remove(id).ok_or_else(Error::not_found)?;
        policy.revision = source
            .policy
            .revision
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "policy revision exhausted"))?;
        source.policy = policy;
        self.generations()
            .commit(snapshot.central.generation, snapshot.central, vec![source])
    }
    pub(in crate::core) fn validate_management(
        &self,
        root: &Path,
        marker: Option<&Marker>,
        source: Option<&str>,
    ) -> Result<()> {
        let context = self.inspect_context(root, marker)?;
        if let Some(id) = source {
            require_source_management(&context, id)
        } else if context.grants.iter().any(|grant| {
            grant.source_id.is_none()
                && grant.selection == "."
                && grant.actions.contains(&Action::Manage)
        }) {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::Authority,
                "management authority is required",
            ))
        }
    }
    /// Management is revalidated while holding the same lock as publication.
    /// The returned bearer must be delivered to the enrolled application only.
    pub(in crate::core) fn register_application(
        &self,
        controller_root: &Path,
        controller: Option<&Marker>,
        request: ApplicationRegistration,
    ) -> Result<RegistrationCredential> {
        let _locks = Lock::transaction(
            &self.layout,
            Vec::new(),
            Vec::new(),
            Instant::now() + Duration::from_secs(5),
        )?;
        self.generations().recover()?;
        let mut index = self.generations().read()?.central;
        let owner = self.context_in(&index, controller_root, controller)?;
        if !owner.grants.iter().any(|g| {
            g.source_id.is_none() && g.selection == "." && g.actions.contains(&Action::Manage)
        }) {
            return Err(Error::new(
                ErrorKind::Authority,
                "management authority required",
            ));
        }
        let root = validate_registration(&request)?;
        if index.applications.values().any(|r| r.root == root) {
            return Err(Error::new(
                ErrorKind::Conflict,
                "application root already enrolled; use explicit rebind or credential reissue",
            ));
        }
        let mut id = [0; 16];
        let mut nonce = [0; 32];
        getrandom::fill(&mut id)
            .and_then(|_| getrandom::fill(&mut nonce))
            .map_err(|_| Error::new(ErrorKind::Internal, "OS random source unavailable"))?;
        let id = utils::hex(&id);
        if index.applications.contains_key(&id) {
            return Err(Error::new(
                ErrorKind::Conflict,
                "registration identity collision",
            ));
        }
        let marker = if request.mode == ApplicationMode::Authoritative {
            Some(self.keys.sign(MarkerClaims {
                schema_version: SCHEMA_VERSION,
                store_id: index.store_id.clone(),
                registration_id: id.clone(),
                registration_revision: 1,
                enrolled_root: root.clone(),
                nonce: utils::hex(&nonce),
                key_generation: index.key_generation,
            })?)
        } else {
            None
        };
        index.applications.insert(
            id.clone(),
            Registration {
                id: id.clone(),
                root,
                revision: 1,
                data_generation: 0,
                mode: request.mode,
                revoked: false,
                grants: request.grants,
                require_verification: request.require_verification,
            },
        );
        self.generations().commit(index.generation, index, vec![])?;
        Ok(RegistrationCredential {
            registration_id: id,
            marker,
        })
    }
    /// Replacement is explicit: it may rebind a root, replace scopes/mode, or
    /// reissue a credential. None revokes access without deleting owned units.
    pub(in crate::core) fn update_application(
        &self,
        controller_root: &Path,
        controller: Option<&Marker>,
        registration_id: &str,
        replacement: Option<ApplicationRegistration>,
    ) -> Result<RegistrationCredential> {
        let _locks = Lock::transaction(
            &self.layout,
            Vec::new(),
            Vec::new(),
            Instant::now() + Duration::from_secs(5),
        )?;
        self.generations().recover()?;
        let mut index = self.generations().read()?.central;
        let owner = self.context_in(&index, controller_root, controller)?;
        if !owner.grants.iter().any(|g| {
            g.source_id.is_none() && g.selection == "." && g.actions.contains(&Action::Manage)
        }) {
            return Err(Error::new(
                ErrorKind::Authority,
                "management authority required",
            ));
        }
        if registration_id == "owner" {
            return Err(Error::new(
                ErrorKind::Conflict,
                "the store controller requires explicit controller recovery",
            ));
        }
        let mut registration = index
            .applications
            .get(registration_id)
            .cloned()
            .ok_or_else(Error::not_found)?;
        registration.revision = registration
            .revision
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "registration revision exhausted"))?;
        registration.data_generation = registration
            .data_generation
            .checked_add(1)
            .ok_or_else(|| Error::new(ErrorKind::Integrity, "application generation exhausted"))?;
        if let Some(request) = replacement {
            let root = validate_registration(&request)?;
            if index
                .applications
                .values()
                .any(|r| r.id != registration.id && r.root == root)
            {
                return Err(Error::new(
                    ErrorKind::Conflict,
                    "application root already enrolled",
                ));
            }
            registration.root = root;
            registration.mode = request.mode;
            registration.grants = request.grants;
            registration.require_verification = request.require_verification;
            registration.revoked = false;
        } else {
            registration.revoked = true;
        }
        let marker = if registration.mode == ApplicationMode::Authoritative && !registration.revoked
        {
            let mut nonce = [0; 32];
            getrandom::fill(&mut nonce)
                .map_err(|_| Error::new(ErrorKind::Internal, "OS random source unavailable"))?;
            Some(self.keys.sign(MarkerClaims {
                schema_version: SCHEMA_VERSION,
                store_id: index.store_id.clone(),
                registration_id: registration.id.clone(),
                registration_revision: registration.revision,
                enrolled_root: registration.root.clone(),
                nonce: utils::hex(&nonce),
                key_generation: index.key_generation,
            })?)
        } else {
            None
        };
        index
            .applications
            .insert(registration.id.clone(), registration);
        self.generations().commit(index.generation, index, vec![])?;
        Ok(RegistrationCredential {
            registration_id: registration_id.into(),
            marker,
        })
    }
}

fn require_source_management(context: &Registration, id: &str) -> Result<()> {
    if context.grants.iter().any(|grant| {
        grant.source_id.is_none()
            && grant.selection == "."
            && grant.actions.contains(&Action::Manage)
    }) {
        Ok(())
    } else {
        crate::core::authority::require_source(context, id, ".", Action::Manage)
    }
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TestEnrollment {
    schema_version: u32,
    store_id: String,
    key_generation: u64,
}

#[cfg(any(test, feature = "test-support"))]
struct TestSecrets {
    layout: Layout,
    key: Zeroizing<[u8; 32]>,
}

#[cfg(any(test, feature = "test-support"))]
impl SecretStore for TestSecrets {
    fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        let path = Path::new("test-store.json");
        if !self.layout.path(path)?.exists() {
            return Ok(None);
        }
        let metadata: TestEnrollment = serde_json::from_slice(&self.layout.read(path, 4096)?)
            .map_err(|_| Error::new(ErrorKind::Integrity, "invalid test-store enrollment"))?;
        let enrollment = Enrollment {
            schema_version: metadata.schema_version,
            store_id: metadata.store_id,
            key_generation: metadata.key_generation,
            master: *self.key,
        };
        serde_json::to_vec(&enrollment)
            .map(Zeroizing::new)
            .map(Some)
            .map_err(|_| Error::new(ErrorKind::Internal, "cannot prepare test enrollment"))
    }
    fn write(&self, bytes: &[u8]) -> Result<()> {
        let enrollment: Enrollment = serde_json::from_slice(bytes)
            .map_err(|_| Error::new(ErrorKind::Integrity, "invalid test-store initialization"))?;
        if enrollment.master != *self.key {
            return Err(Error::new(ErrorKind::Integrity, "test key mismatch"));
        }
        let metadata = TestEnrollment {
            schema_version: enrollment.schema_version,
            store_id: enrollment.store_id.clone(),
            key_generation: enrollment.key_generation,
        };
        let bytes = serde_json::to_vec(&metadata)
            .map_err(|_| Error::new(ErrorKind::Internal, "cannot encode test enrollment"))?;
        self.layout
            .create_file(Path::new("test-store.json"), &bytes)
    }
    #[cfg(test)]
    fn delete(&self) -> Result<()> {
        self.layout.remove_file(Path::new("test-store.json"))
    }
}

fn validate_registration(request: &ApplicationRegistration) -> Result<String> {
    let root = std::fs::canonicalize(&request.root)
        .map_err(|_| Error::new(ErrorKind::Config, "application root must exist"))?;
    if !root.is_dir() {
        return Err(Error::new(
            ErrorKind::Config,
            "application root must be a directory",
        ));
    }
    let root = root
        .to_str()
        .ok_or_else(|| Error::new(ErrorKind::Config, "application root must be UTF-8"))?
        .to_owned();
    for grant in &request.grants {
        crate::core::recipes::validate_selection(&grant.selection)?;
        if grant.actions.is_empty()
            || grant.source_id.as_ref().is_some_and(|id| {
                id.len() != 64
                    || !id
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            })
        {
            return Err(Error::new(ErrorKind::Config, "invalid resource grant"));
        }
        if grant.source_id.is_none()
            && (grant.actions.len() != 1
                || !grant.actions.contains(&Action::Manage)
                || grant.selection != ".")
        {
            return Err(Error::new(
                ErrorKind::Config,
                "resource grants require a canonical source identity",
            ));
        }
        for destination in &grant.destinations {
            let path = std::path::PathBuf::from(destination);
            if !path.is_absolute()
                || std::fs::canonicalize(&path).ok().as_deref() != Some(path.as_path())
            {
                return Err(Error::new(
                    ErrorKind::Config,
                    "grant destination must be an existing canonical absolute root",
                ));
            }
        }
    }
    Ok(root)
}

#[cfg(test)]
#[path = "acquisition_tests.rs"]
mod acquisition_tests;

#[cfg(test)]
mod tests {
    use super::super::keyring::testing::MemorySecrets;
    use super::*;
    #[test]
    fn management_registration_rebind_reissue_and_revocation_are_current() {
        let home = tempfile::tempdir().unwrap();
        let app_a = tempfile::tempdir().unwrap();
        let app_b = tempfile::tempdir().unwrap();
        let moved = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        let secrets = MemorySecrets::default();
        let store = Session::initialize(layout.clone(), &secrets).unwrap();
        let owner: Marker = serde_json::from_slice(
            &std::fs::read(layout.root().join("owner.saucepanhash")).unwrap(),
        )
        .unwrap();
        let request = |root: &Path, mode| ApplicationRegistration {
            root: root.to_string_lossy().into(),
            mode,
            grants: vec![],
            require_verification: false,
        };
        let credential = store
            .register_application(
                layout.root(),
                Some(&owner),
                request(app_a.path(), ApplicationMode::Authoritative),
            )
            .unwrap();
        let marker = credential.marker.as_ref().unwrap();
        assert!(store.inspect_context(app_a.path(), Some(marker)).is_ok());
        assert!(
            matches!(store.inspect_context(app_a.path(), None), Err(e) if e.kind == ErrorKind::Authority)
        );
        let ordinary = store
            .register_application(
                layout.root(),
                Some(&owner),
                request(app_b.path(), ApplicationMode::Ordinary),
            )
            .unwrap();
        assert!(ordinary.marker.is_none());
        assert!(store.inspect_context(app_b.path(), None).is_ok());
        // An optional credential must still identify the selected context.
        assert!(store.inspect_context(app_b.path(), Some(marker)).is_err());
        assert!(store.inspect_context(app_a.path(), Some(marker)).is_ok());
        let mut edited = marker.clone();
        edited.claims.enrolled_root = app_b.path().to_string_lossy().into();
        assert!(store.inspect_context(app_a.path(), Some(&edited)).is_err());
        assert!(store.inspect_context(moved.path(), Some(marker)).is_err());
        let before = store.read_index().unwrap().generation;
        assert!(
            matches!(store.register_application(app_a.path(), Some(marker), request(Path::new("missing-target-root"), ApplicationMode::Ordinary)), Err(e) if e.kind == ErrorKind::Authority)
        );
        assert!(
            matches!(store.update_application(app_b.path(), None, &credential.registration_id, None), Err(e) if e.kind == ErrorKind::Authority)
        );
        assert_eq!(store.read_index().unwrap().generation, before);
        let reissued = store
            .update_application(
                layout.root(),
                Some(&owner),
                &credential.registration_id,
                Some(request(app_a.path(), ApplicationMode::Authoritative)),
            )
            .unwrap();
        assert!(store.inspect_context(app_a.path(), Some(marker)).is_err());
        assert!(
            store
                .inspect_context(app_a.path(), reissued.marker.as_ref())
                .is_ok()
        );
        let rebound = store
            .update_application(
                layout.root(),
                Some(&owner),
                &credential.registration_id,
                Some(request(moved.path(), ApplicationMode::Authoritative)),
            )
            .unwrap();
        assert!(
            store
                .inspect_context(app_a.path(), reissued.marker.as_ref())
                .is_err()
        );
        assert!(
            store
                .inspect_context(moved.path(), reissued.marker.as_ref())
                .is_err()
        );
        assert!(
            store
                .inspect_context(moved.path(), rebound.marker.as_ref())
                .is_ok()
        );
        // Another process/session sees the current revocation, never cached grants.
        let other_session = Session::open(layout.clone(), &secrets).unwrap();
        store
            .update_application(
                layout.root(),
                Some(&owner),
                &credential.registration_id,
                None,
            )
            .unwrap();
        assert!(
            other_session
                .inspect_context(moved.path(), rebound.marker.as_ref())
                .is_err()
        );
        assert!(store.inspect_context(layout.root(), Some(&owner)).is_ok());
        assert!(store.inspect_context(app_b.path(), None).is_ok());
    }
    #[test]
    fn explicit_initialize_and_open_do_not_use_plaintext_fallback() {
        let home = tempfile::tempdir().unwrap();
        let secrets = MemorySecrets::default();
        let layout = Layout::under(home.path()).unwrap();
        assert!(Session::open(layout.clone(), &secrets).is_err());
        assert!(!layout.root().exists());
        let store = Session::initialize(layout.clone(), &secrets).unwrap();
        store.check_store().unwrap();
        let owner: Marker = serde_json::from_slice(
            &std::fs::read(layout.root().join("owner.saucepanhash")).unwrap(),
        )
        .unwrap();
        assert!(store.inspect_context(layout.root(), Some(&owner)).is_ok());
        assert!(store.inspect_context(layout.root(), None).is_err());
        Session::open(layout.clone(), &secrets).unwrap();
        let raw = std::fs::read(layout.root().join("index.json.enc")).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("applications"));
        assert!(!layout.root().join("index.json").exists());
        assert!(Session::initialize(layout.clone(), &secrets).is_err());
        std::fs::remove_file(layout.root().join("index.json.enc")).unwrap();
        assert!(
            matches!(Session::open(layout.clone(), &secrets), Err(e) if e.kind == ErrorKind::Authority)
        );
        assert!(Session::initialize(layout, &secrets).is_err());
    }
    #[test]
    fn missing_or_locked_keyring_never_overwrites_existing_state() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        let locked = MemorySecrets {
            unavailable: true,
            ..Default::default()
        };
        assert!(Session::initialize(layout.clone(), &locked).is_err());
        assert!(!layout.root().exists());
        let secrets = MemorySecrets::default();
        Session::initialize(layout.clone(), &secrets).unwrap();
        let before = std::fs::read(layout.root().join("index.json.enc")).unwrap();
        secrets.delete().unwrap();
        assert!(Session::open(layout.clone(), &secrets).is_err());
        assert!(Session::initialize(layout.clone(), &secrets).is_err());
        assert_eq!(
            std::fs::read(layout.root().join("index.json.enc")).unwrap(),
            before
        );
    }
    #[test]
    fn corrupted_index_is_not_reenrolled() {
        let home = tempfile::tempdir().unwrap();
        let layout = Layout::under(home.path()).unwrap();
        let secrets = MemorySecrets::default();
        Session::initialize(layout.clone(), &secrets).unwrap();
        std::fs::write(layout.root().join("index.json.enc"), b"{}").unwrap();
        assert!(
            matches!(Session::open(layout.clone(), &secrets), Err(e) if e.kind == ErrorKind::Integrity)
        );
        assert_eq!(
            std::fs::read(layout.root().join("index.json.enc")).unwrap(),
            b"{}"
        );
    }
}
