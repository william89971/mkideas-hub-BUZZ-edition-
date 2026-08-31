use thiserror::Error;

/// Result type used by the migration planning library.
pub type Result<T> = std::result::Result<T, MigrationError>;

/// Failures raised before any source or destination mutation is possible.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum MigrationError {
    /// A local, read-only bundle file could not be opened or parsed.
    #[error("bundle I/O failed: {0}")]
    BundleIo(String),
    /// A source row violates the supported legacy contract.
    #[error("invalid source row: {0}")]
    InvalidSource(String),
    /// Two source rows use the same stable table identity.
    #[error("duplicate source row {table}:{key}")]
    DuplicateSource {
        /// Legacy table.
        table: String,
        /// Conflicting primary or composite key.
        key: String,
    },
    /// A manifest, checkpoint, plan, or report could not be serialized.
    #[error("serialization failed: {0}")]
    Serialization(String),
    /// The export manifest is internally inconsistent.
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
    /// A bundle path is absolute or escapes its bundle root.
    #[error("unsafe bundle path: {0}")]
    UnsafePath(String),
    /// A source record contains a field that must never enter a migration bundle.
    #[error("forbidden source fields: {0}")]
    ForbiddenFields(String),
    /// A legacy status has no explicit destination mapping.
    #[error("unsupported {domain} status: {status}")]
    UnsupportedStatus {
        /// Domain whose status failed conversion.
        domain: &'static str,
        /// Source status.
        status: String,
    },
    /// Two plan units use the same source coordinate.
    #[error("duplicate migration unit: {0}")]
    DuplicateUnit(String),
    /// A unit depends on a source coordinate absent from the plan.
    #[error("missing dependency {dependency} required by {unit}")]
    MissingDependency {
        /// Unit with the missing dependency.
        unit: String,
        /// Missing source coordinate.
        dependency: String,
    },
    /// The dependency graph contains a cycle.
    #[error("migration dependency cycle: {0}")]
    DependencyCycle(String),
    /// A receipt or checkpoint conflicts with the immutable plan.
    #[error("migration identity conflict: {0}")]
    IdentityConflict(String),
}
