#[cfg(feature = "flatpak")]
pub mod flathub;
#[cfg(feature = "github")]
mod github;
mod mac64;
mod mac_arm64;
#[cfg(feature = "pypi")]
mod pypi;
mod win32;
mod win64;

#[cfg(feature = "flatpak")]
pub use flathub::{FlathubArtifactType, FlathubBeta, FlathubBranch, FlathubStable};
#[cfg(feature = "github")]
pub use github::GithubArtifactType;
pub use mac64::Mac64ArtifactType;
pub use mac_arm64::MacArm64ArtifactType;
#[cfg(feature = "pypi")]
pub use pypi::PypiArtifactType;
pub use win32::Win32ArtifactType;
pub use win64::Win64ArtifactType;
