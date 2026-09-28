use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DiskFormat {
    Sparse,
    SplitSparse,
    Preallocated,
    SplitPreallocated,
}

impl DiskFormat {
    pub(super) fn from_options(preallocate: bool, split: bool) -> Self {
        match (preallocate, split) {
            (false, false) => Self::Sparse,
            (false, true) => Self::SplitSparse,
            (true, false) => Self::Preallocated,
            (true, true) => Self::SplitPreallocated,
        }
    }

    // vmware-vdiskmanager represents each VMDK format with a numeric -t code
    pub(super) fn vdisk_type(self) -> &'static str {
        match self {
            Self::Sparse => "0",
            Self::SplitSparse => "1",
            Self::Preallocated => "2",
            Self::SplitPreallocated => "3",
        }
    }
}

impl fmt::Display for DiskFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sparse => formatter.write_str("sparse"),
            Self::SplitSparse => formatter.write_str("split sparse"),
            Self::Preallocated => formatter.write_str("preallocated"),
            Self::SplitPreallocated => formatter.write_str("split preallocated"),
        }
    }
}
