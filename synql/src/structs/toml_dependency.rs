//! Submodule defining the `TomlDependency` struct.

use crate::Error;

/// Struct representing a TOML dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TomlDependency {
    /// Name of the dependency.
    name: String,
    /// Version requirements for the dependency.
    version: Option<String>,
    /// Git repository URL.
    git: Option<String>,
    /// Branch of the git repository.
    branch: Option<String>,
    /// Features to enable.
    features: Vec<String>,
    /// Whether the dependency is optional.
    optional: bool,
    /// Whether to use the workspace version.
    workspace: bool,
    /// Whether to use default features.
    default_features: Option<bool>,
    /// Path to the dependency.
    path: Option<String>,
}

impl TomlDependency {
    /// Creates a new `TomlDependency` instance.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: None,
            git: None,
            branch: None,
            features: Vec::new(),
            optional: false,
            workspace: false,
            default_features: None,
            path: None,
        }
    }

    /// Sets the version requirements for the dependency.
    ///
    /// # Errors
    ///
    /// Returns an error if the dependency is a workspace dependency.
    pub fn version(mut self, version: impl Into<String>) -> Result<Self, Error> {
        if self.workspace {
            return Err(Error::InvalidTomlDependency(
                "Cannot set version for a workspace dependency".to_string(),
            ));
        }
        self.version = Some(version.into());
        Ok(self)
    }

    /// Sets the git repository URL for the dependency.
    ///
    /// # Errors
    ///
    /// Returns an error if the dependency is a workspace dependency.
    pub fn git(
        mut self,
        git: impl Into<String>,
        branch: Option<impl Into<String>>,
    ) -> Result<Self, Error> {
        if self.workspace {
            return Err(Error::InvalidTomlDependency(
                "Cannot set git for a workspace dependency".to_string(),
            ));
        }
        self.git = Some(git.into());
        self.branch = branch.map(Into::into);
        Ok(self)
    }

    /// Returns the branch of the git repository for the dependency.
    #[must_use]
    pub fn get_branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }

    /// Returns the version requirements for the dependency.
    #[must_use]
    pub fn get_version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Returns the git repository URL for the dependency.
    #[must_use]
    pub fn get_git(&self) -> Option<&str> {
        self.git.as_deref()
    }

    /// Returns the features enabled for the dependency.
    #[must_use]
    pub fn features(&self) -> &[String] {
        &self.features
    }

    /// Returns whether the dependency is optional.
    #[must_use]
    pub fn is_optional(&self) -> bool {
        self.optional
    }

    /// Returns whether the dependency uses the workspace version.
    #[must_use]
    pub fn is_workspace(&self) -> bool {
        self.workspace
    }

    /// Returns the default features setting for the dependency.
    #[must_use]
    pub fn get_default_features(&self) -> Option<bool> {
        self.default_features
    }

    /// Adds a feature to the dependency.
    #[must_use]
    pub fn feature(mut self, feature: impl Into<String>) -> Self {
        self.features.push(feature.into());
        self
    }

    /// Sets whether the dependency is optional.
    #[must_use]
    pub fn optional(mut self, optional: bool) -> Self {
        self.optional = optional;
        self
    }

    /// Sets whether to use the workspace version.
    ///
    /// # Errors
    ///
    /// Returns an error if incompatible attributes are already set.
    pub fn workspace(mut self, workspace: bool) -> Result<Self, Error> {
        if workspace {
            if self.version.is_some() {
                return Err(Error::InvalidTomlDependency(
                    "Cannot set workspace to true when version is set".to_string(),
                ));
            }
            if self.git.is_some() {
                return Err(Error::InvalidTomlDependency(
                    "Cannot set workspace to true when git is set".to_string(),
                ));
            }
            if self.branch.is_some() {
                return Err(Error::InvalidTomlDependency(
                    "Cannot set workspace to true when branch is set".to_string(),
                ));
            }
            if self.path.is_some() {
                return Err(Error::InvalidTomlDependency(
                    "Cannot set workspace to true when path is set".to_string(),
                ));
            }
        }
        self.workspace = workspace;
        Ok(self)
    }

    /// Sets whether to use default features.
    #[must_use]
    pub fn default_features(mut self, default_features: bool) -> Self {
        self.default_features = Some(default_features);
        self
    }

    /// Sets the path to the dependency.
    ///
    /// # Errors
    ///
    /// Returns an error if the dependency is a workspace dependency.
    pub fn path(mut self, path: impl Into<String>) -> Result<Self, Error> {
        if self.workspace {
            return Err(Error::InvalidTomlDependency(
                "Cannot set path for a workspace dependency".to_string(),
            ));
        }
        self.path = Some(path.into());
        Ok(self)
    }

    /// Converts the struct into a struct that only has the workspace=true.
    #[must_use]
    pub fn into_workspace_dependency(mut self) -> Self {
        self.workspace = true;
        self.version = None;
        self.git = None;
        self.branch = None;
        self.path = None;
        self
    }

    /// Returns the name of the dependency.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the path of the dependency.
    #[must_use]
    pub fn get_path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Returns the `[patch.crates-io]` entry redirecting this published
    /// dependency to its git source, or `None` unless it has both a version
    /// and a git source.
    #[must_use]
    pub fn crates_io_patch(&self) -> Option<Self> {
        self.version.as_ref()?;
        Some(Self {
            git: Some(self.git.clone()?),
            branch: self.branch.clone(),
            ..Self::new(self.name.clone())
        })
    }
}

impl std::fmt::Display for TomlDependency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;

        if self.workspace {
            write!(f, ".workspace = true")?;
            return Ok(());
        }

        write!(f, " = {{ ")?;

        let mut parts = Vec::new();

        if let Some(version) = &self.version {
            parts.push(format!("version = \"{version}\""));
        }

        if let Some(git) = &self.git {
            parts.push(format!("git = \"{git}\""));
        }

        if let Some(branch) = &self.branch {
            parts.push(format!("branch = \"{branch}\""));
        }

        if let Some(path) = &self.path {
            parts.push(format!("path = \"{path}\""));
        }

        if !self.features.is_empty() {
            let features_str = self
                .features
                .iter()
                .map(|feature| format!("\"{feature}\""))
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("features = [{features_str}]"));
        }

        if self.optional {
            parts.push("optional = true".to_string());
        }

        if let Some(default_features) = self.default_features {
            parts.push(format!("default-features = {default_features}"));
        }

        write!(f, "{}", parts.join(", "))?;
        write!(f, " }}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_dependency() {
        let dep = TomlDependency::new("serde");
        assert_eq!(dep.to_string(), "serde = {  }");
    }

    #[test]
    fn test_version() {
        let dep = TomlDependency::new("serde").version("1.0").unwrap();
        assert_eq!(dep.to_string(), "serde = { version = \"1.0\" }");
    }

    #[test]
    fn test_git_branch() {
        let dep = TomlDependency::new("geometric-traits")
            .git("https://github.com/earth-metabolome-initiative/geometric-traits", Some("main"))
            .unwrap();
        assert_eq!(
            dep.to_string(),
            "geometric-traits = { git = \"https://github.com/earth-metabolome-initiative/geometric-traits\", branch = \"main\" }"
        );
    }

    #[test]
    fn test_features() {
        let dep =
            TomlDependency::new("serde").version("1.0").unwrap().feature("derive").feature("alloc");
        assert_eq!(
            dep.to_string(),
            "serde = { version = \"1.0\", features = [\"derive\", \"alloc\"] }"
        );
    }

    #[test]
    fn test_optional() {
        let dep = TomlDependency::new("serde").version("1.0").unwrap().optional(true);
        assert_eq!(dep.to_string(), "serde = { version = \"1.0\", optional = true }");
    }

    #[test]
    fn test_workspace() {
        let dep = TomlDependency::new("serde").workspace(true).unwrap();
        assert_eq!(dep.to_string(), "serde.workspace = true");
    }

    #[test]
    fn test_default_features() {
        let dep = TomlDependency::new("serde").version("1.0").unwrap().default_features(false);
        assert_eq!(dep.to_string(), "serde = { version = \"1.0\", default-features = false }");
    }

    #[test]
    fn test_path() {
        let dep = TomlDependency::new("local-crate").path("../local").unwrap();
        assert_eq!(dep.to_string(), "local-crate = { path = \"../local\" }");
    }

    #[test]
    fn test_complex_combination() {
        let dep = TomlDependency::new("complex")
            .version("2.0")
            .unwrap()
            .optional(true)
            .feature("feat1")
            .default_features(false);

        assert_eq!(
            dep.to_string(),
            "complex = { version = \"2.0\", features = [\"feat1\"], optional = true, default-features = false }"
        );
    }

    #[test]
    fn test_crates_io_patch_keeps_only_the_git_source() {
        let dep = TomlDependency::new("diesel")
            .version("2.3.13")
            .unwrap()
            .git("https://github.com/diesel-rs/diesel", Some("main"))
            .unwrap()
            .feature("postgres")
            .default_features(false);
        assert_eq!(
            dep.crates_io_patch().unwrap().to_string(),
            "diesel = { git = \"https://github.com/diesel-rs/diesel\", branch = \"main\" }"
        );

        let without_branch = TomlDependency::new("diesel")
            .version("2.3.13")
            .unwrap()
            .git("https://github.com/diesel-rs/diesel", None::<String>)
            .unwrap();
        assert_eq!(
            without_branch.crates_io_patch().unwrap().to_string(),
            "diesel = { git = \"https://github.com/diesel-rs/diesel\" }"
        );
    }

    #[test]
    fn test_crates_io_patch_needs_version_and_git() {
        let git_only = TomlDependency::new("diesel-builders")
            .git("https://github.com/connetto-rs/diesel-builders", Some("main"))
            .unwrap();
        assert_eq!(git_only.crates_io_patch(), None);
        assert_eq!(TomlDependency::new("serde").version("1").unwrap().crates_io_patch(), None);
    }
}
