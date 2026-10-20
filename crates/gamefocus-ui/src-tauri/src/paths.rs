//! Re-export path helpers from core so the UI crate stays thin.

pub use gamefocus_core::{
    default_snapshot_path, discover_profiles, resolve_profile_path,
};
