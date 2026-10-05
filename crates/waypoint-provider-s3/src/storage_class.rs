// The storage class of an object, and the attributes a listing carries beyond a `ScannedEntry`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Where an object is kept, which decides what it costs and whether it can be read at once.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StorageClass {
    Standard,
    StandardIa,
    OnezoneIa,
    IntelligentTiering,
    ReducedRedundancy,
    /// Instant-retrieval archive: readable at once.
    GlacierInstantRetrieval,
    /// Flexible-retrieval archive: a restore is needed before it can be read.
    Glacier,
    /// Deep archive: a restore is needed before it can be read.
    DeepArchive,
    ExpressOnezone,
    /// A class this crate has no name for (another service's, or a newer AWS one), as the service
    /// spelled it.
    Other(String),
}

impl StorageClass {
    /// The class a service spelled `name` (the API names, `STANDARD_IA`).
    pub fn from_api_name(name: &str) -> Self {
        match name {
            "STANDARD" => Self::Standard,
            "STANDARD_IA" => Self::StandardIa,
            "ONEZONE_IA" => Self::OnezoneIa,
            "INTELLIGENT_TIERING" => Self::IntelligentTiering,
            "REDUCED_REDUNDANCY" => Self::ReducedRedundancy,
            "GLACIER_IR" => Self::GlacierInstantRetrieval,
            "GLACIER" => Self::Glacier,
            "DEEP_ARCHIVE" => Self::DeepArchive,
            "EXPRESS_ONEZONE" => Self::ExpressOnezone,
            other => Self::Other(other.to_owned()),
        }
    }

    pub fn api_name(&self) -> &str {
        match self {
            Self::Standard => "STANDARD",
            Self::StandardIa => "STANDARD_IA",
            Self::OnezoneIa => "ONEZONE_IA",
            Self::IntelligentTiering => "INTELLIGENT_TIERING",
            Self::ReducedRedundancy => "REDUCED_REDUNDANCY",
            Self::GlacierInstantRetrieval => "GLACIER_IR",
            Self::Glacier => "GLACIER",
            Self::DeepArchive => "DEEP_ARCHIVE",
            Self::ExpressOnezone => "EXPRESS_ONEZONE",
            Self::Other(name) => name,
        }
    }

    /// The class as the Storage class column writes it (the catalogue translates the known ones;
    /// this is the English fallback and the whole text of an unknown class).
    pub fn label(&self) -> String {
        match self {
            Self::Standard => "Standard".to_owned(),
            Self::StandardIa => "Standard-IA".to_owned(),
            Self::OnezoneIa => "One Zone-IA".to_owned(),
            Self::IntelligentTiering => "Intelligent-Tiering".to_owned(),
            Self::ReducedRedundancy => "Reduced redundancy".to_owned(),
            Self::GlacierInstantRetrieval => "Glacier Instant Retrieval".to_owned(),
            Self::Glacier => "Glacier Flexible Retrieval".to_owned(),
            Self::DeepArchive => "Glacier Deep Archive".to_owned(),
            Self::ExpressOnezone => "Express One Zone".to_owned(),
            Self::Other(name) => name.clone(),
        }
    }

    /// Whether reading an object of this class needs a restore first. Waypoint never starts one.
    pub fn needs_restore(&self) -> bool {
        matches!(self, Self::Glacier | Self::DeepArchive)
    }
}

/// What a listing knows about an object besides what every provider reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectAttributes {
    /// `Standard` when the service reports no class (a store with only one).
    pub storage_class: StorageClass,
    pub etag: Option<String>,
}

impl ObjectAttributes {
    pub fn archived(&self) -> bool {
        self.storage_class.needs_restore()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_names_round_trip_and_unknown_ones_survive() {
        for name in [
            "STANDARD",
            "STANDARD_IA",
            "ONEZONE_IA",
            "INTELLIGENT_TIERING",
            "REDUCED_REDUNDANCY",
            "GLACIER_IR",
            "GLACIER",
            "DEEP_ARCHIVE",
            "EXPRESS_ONEZONE",
            "COLD",
        ] {
            assert_eq!(StorageClass::from_api_name(name).api_name(), name);
        }
        assert_eq!(
            StorageClass::from_api_name("COLD"),
            StorageClass::Other("COLD".to_owned())
        );
        assert_eq!(StorageClass::Other("COLD".into()).label(), "COLD");
    }

    #[test]
    fn only_flexible_and_deep_archive_need_a_restore() {
        assert!(StorageClass::Glacier.needs_restore());
        assert!(StorageClass::DeepArchive.needs_restore());
        assert!(!StorageClass::GlacierInstantRetrieval.needs_restore());
        assert!(!StorageClass::Standard.needs_restore());
        assert!(ObjectAttributes {
            storage_class: StorageClass::DeepArchive,
            etag: None
        }
        .archived());
    }
}
