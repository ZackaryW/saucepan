//! The native boundary, independent of CLI presentation.
mod acquisition;
mod authority;
mod materialization;
pub mod models;
mod policies;
mod recipes;
mod snapshots;
mod sources;
mod store;

/// Access to the OS user's native store. Explicit custom path/key construction
/// is available only in builds that enable the `test-support` feature.
pub struct Service {
    session: store::Session,
}

impl Service {
    /// Read only this application's currently inspectable installed metadata.
    pub fn application_view(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
    ) -> models::Result<models::ApplicationView> {
        self.session.application_view(root, marker)
    }

    /// Recheck current scope and origins before reusing cached installed data.
    pub fn application_generation(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
    ) -> models::Result<models::ApplicationGeneration> {
        self.session.application_generation(root, marker)
    }

    /// Hidden installed names have the same response as absent names.
    pub fn unit_view(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        name: &str,
    ) -> models::Result<models::UnitView> {
        self.session.unit_view(root, marker, name)
    }

    /// Make an independent mirror of a named unit or explicit materialization.
    pub fn mirror(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        target: &models::BindingTarget,
        destination: &std::path::Path,
        verification: &models::VerificationRequest,
    ) -> models::Result<models::MirrorBinding> {
        self.session
            .mirror(root, marker, target, destination, verification)
    }
    pub fn mirror_path(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        target: &models::BindingTarget,
        destination: &std::path::Path,
        verification: &models::VerificationRequest,
    ) -> models::Result<std::path::PathBuf> {
        self.session
            .mirror_path(root, marker, target, destination, verification)
    }
    pub fn remove_mirror(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        target: &models::BindingTarget,
        destination: &std::path::Path,
    ) -> models::Result<()> {
        self.session
            .remove_mirror(root, marker, target, destination)
    }
    /// Materialize an exact recorded artifact without installing a named unit,
    /// refreshing a branch, or changing the recipe's current pointer.
    pub fn materialize(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        recipe: &models::Recipe,
        artifact_id: &str,
        verification: &models::VerificationRequest,
    ) -> models::Result<models::MaterializedArtifact> {
        self.session
            .materialize(root, marker, recipe, artifact_id, verification)
    }
    pub fn materialization_path(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        id: &str,
        verification: &models::VerificationRequest,
    ) -> models::Result<std::path::PathBuf> {
        self.session
            .materialization_path(root, marker, id, verification)
    }
    pub fn release_materialization(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        id: &str,
    ) -> models::Result<()> {
        self.session.release_materialization(root, marker, id)
    }
    /// Refresh a named installation using its recorded declarative recipe.
    pub fn update(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        name: &str,
        verification: &models::VerificationRequest,
    ) -> models::Result<models::UnitBinding> {
        self.session.update(root, marker, name, verification)
    }
    /// Remove only the caller's binding and unchanged owned materialization.
    pub fn uninstall(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        name: &str,
    ) -> models::Result<()> {
        self.session.uninstall(root, marker, name)
    }
    /// Acquire an explicit recipe and publish an independent manifest-named
    /// installation. A failed replacement preserves its previous binding.
    pub fn install(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        recipe: &models::Recipe,
        verification: &models::VerificationRequest,
    ) -> models::Result<models::UnitBinding> {
        self.session.install(root, marker, recipe, verification)
    }
    /// Resolve the caller's installed directory with current scope/origin checks.
    pub fn path(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        name: &str,
        verification: &models::VerificationRequest,
    ) -> models::Result<std::path::PathBuf> {
        self.session.unit_path(root, marker, name, verification)
    }
    /// Check management scope before an adapter reads a management request.
    pub fn validate_management(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        source: Option<&str>,
    ) -> models::Result<()> {
        self.session.validate_management(root, marker, source)
    }
    /// Authenticate the selected application before an adapter reads preferences.
    /// Resource operations still revalidate their current scopes independently.
    pub fn validate_context(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
    ) -> models::Result<()> {
        self.session.inspect_context(root, marker).map(|_| ())
    }
    /// Change shared retention/verification defaults through management scope.
    /// The store assigns the next revision; old history is not implicitly erased.
    pub fn set_source_policy(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        source_id: &str,
        policy: models::CachePolicy,
    ) -> models::Result<()> {
        self.session
            .set_source_policy(root, marker, source_id, policy)
    }
    /// Read exact current/history content without advancing a tracked revision.
    /// Uncached current is reconstructed from its pinned inputs. Successful
    /// historical reads refresh its source-wide LRU order.
    pub fn read_archive(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        recipe: &models::Recipe,
        artifact_id: &str,
        verification: &models::VerificationRequest,
    ) -> models::Result<models::ArchiveRead> {
        acquisition::read_archive(
            &self.session,
            root,
            marker,
            recipe,
            artifact_id,
            verification,
        )
    }
    /// Acquire a recipe through current application scope, Git origin checks,
    /// committed export and the recoverable source-wide rolling history.
    pub fn acquire(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        recipe: &models::Recipe,
        verification: &models::VerificationRequest,
    ) -> models::Result<models::ArtifactHandle> {
        acquisition::acquire(&self.session, root, marker, recipe, verification)
    }
    /// Create a new isolated encrypted test store at the exact supplied path.
    /// The 32-byte key is supplied by the caller and is never saved in the store.
    #[cfg(feature = "test-support")]
    pub fn initialize_test(root: &std::path::Path, key: [u8; 32]) -> models::Result<Self> {
        store::Session::initialize_test(root, key).map(|session| Self { session })
    }
    /// Open an existing test store with its original key; never initializes or
    /// falls back to the OS keyring when the path or key is wrong.
    #[cfg(feature = "test-support")]
    pub fn open_test(root: &std::path::Path, key: [u8; 32]) -> models::Result<Self> {
        store::Session::open_test(root, key).map(|session| Self { session })
    }
    /// Describe a caller-supplied source for recipe/grant construction. This
    /// performs no fetch or lookup of protected source records.
    pub fn describe_source(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        locator: &models::SourceLocator,
    ) -> models::Result<models::SourceIdentity> {
        let context = self.session.inspect_context(root, marker)?;
        sources::identify(locator, std::path::Path::new(&context.root))
    }
    pub fn open_user() -> models::Result<Self> {
        store::Session::open_user().map(|session| Self { session })
    }
    /// Explicit trusted first initialization, never an implicit open fallback.
    pub fn initialize_user() -> models::Result<Self> {
        store::Session::initialize_user().map(|session| Self { session })
    }
    /// Probe store consistency without exposing global metadata.
    pub fn check_store(&self) -> models::Result<()> {
        self.session.check_store()
    }
    /// Enroll an existing application root using current management authority.
    /// Deliver the returned marker as `.saucepanhash` at that root; the native
    /// API returns it directly so an adapter can use a protected delivery path.
    pub fn register_application(
        &self,
        controller_root: &std::path::Path,
        controller: Option<&models::Marker>,
        request: models::ApplicationRegistration,
    ) -> models::Result<models::RegistrationCredential> {
        self.session
            .register_application(controller_root, controller, request)
    }
    /// Explicit management replacement/rebind/reissue, or revocation with None.
    /// Every change invalidates previously issued application credentials.
    pub fn update_application(
        &self,
        controller_root: &std::path::Path,
        controller: Option<&models::Marker>,
        registration_id: &str,
        replacement: Option<models::ApplicationRegistration>,
    ) -> models::Result<models::RegistrationCredential> {
        self.session
            .update_application(controller_root, controller, registration_id, replacement)
    }

    /// Evaluate source preferences only after validating current registration.
    /// The source policy is an input to this preview; it cannot mutate the store.
    pub fn preview_policy(
        &self,
        root: &std::path::Path,
        marker: Option<&models::Marker>,
        source: &models::CachePolicy,
        request: &models::VerificationRequest,
    ) -> models::Result<models::EffectivePolicy> {
        let registration = self.session.inspect_context(root, marker)?;
        policies::evaluate(source, registration.require_verification, request)
    }
}

/// Validate a declarative request without fetching, reading configuration, or
/// creating state. Validation does not confer permission to acquire the recipe.
pub fn validate_recipe(recipe: &models::Recipe) -> models::Result<()> {
    acquisition::preflight(recipe)
}

/// Parse and validate an untrusted recipe without acquisition effects.
pub fn parse_recipe(bytes: &[u8]) -> models::Result<models::Recipe> {
    if bytes.len() > 1024 * 1024 {
        return Err(models::Error::new(
            models::ErrorKind::Config,
            "recipe exceeds size limit",
        ));
    }
    let recipe: models::Recipe = serde_json::from_slice(bytes)
        .map_err(|_| models::Error::new(models::ErrorKind::Config, "invalid recipe document"))?;
    validate_recipe(&recipe)?;
    Ok(recipe)
}
