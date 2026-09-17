//! Lane selection and lane observation for the GF(2^8) axpy confirmation
//! (jit:ad2a6a58).
//!
//! Both arms of a pair are one executable. The environment names the lane
//! `FieldVec::axpy` is held on and the element representation the consumer
//! holds; nothing else distinguishes a baseline execution from a candidate
//! one. The lane is reported from the shipped crate's own witness after the
//! measured calls, never from the setting that requested it.

use gf2_core::gf2m::{force_scalar_gf256_table, gf256_table_builds, last_gf256_table_lane};

/// Names the lane every GF(2^8) call of this process is held on.
pub const LANE_VAR: &str = "GF2_GF256_LANE";

/// Names the GF(2^8) element representation the consumer holds.
pub const REPR_VAR: &str = "GF2_GF256_REPR";

/// The lane a pair position runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// The route the library takes without the cached product table: every
    /// call declines `gf256_table_dispatch` and runs the consumer's scalar
    /// element loop.
    Scalar,
    /// The shipped accelerated lane, free to reach the cached table.
    Table,
}

impl Lane {
    /// The lane [`LANE_VAR`] names.
    pub fn from_env() -> Result<Self, String> {
        match std::env::var(LANE_VAR).as_deref() {
            Ok("scalar") => Ok(Lane::Scalar),
            Ok("table") => Ok(Lane::Table),
            Ok(other) => Err(format!("unknown {LANE_VAR} {other:?}")),
            Err(_) => Err(format!("{LANE_VAR} is unset")),
        }
    }

    /// Holds every GF(2^8) caller in this process on this lane.
    ///
    /// One process-global switch covers every caller of the shipped
    /// dispatch, so a baseline execution reaches the table lane nowhere.
    pub fn apply(self) {
        force_scalar_gf256_table(self == Lane::Scalar);
    }
}

/// The element representation a cell's consumer holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Representation {
    /// `Gf2mElement` over the runtime field `Gf2mField::gf256()`.
    Element,
    /// `Gf2mWide<1, Gf256x11d>`, the compile-time configured element.
    Wide,
}

impl Representation {
    /// The representation [`REPR_VAR`] names.
    pub fn from_env() -> Result<Self, String> {
        match std::env::var(REPR_VAR).as_deref() {
            Ok("element") => Ok(Representation::Element),
            Ok("wide") => Ok(Representation::Wide),
            Ok(other) => Err(format!("unknown {REPR_VAR} {other:?}")),
            Err(_) => Err(format!("{REPR_VAR} is unset")),
        }
    }
}

/// The lane the most recent GF(2^8) dispatch on this thread took, with the
/// number of product tables this process built.
///
/// Read after the timed windows: a `cold` cell performs its first dispatch
/// inside its first window, so no earlier reading describes the measured
/// calls. The build count separates a first-touch execution, which pays one
/// table build, from one that finds the table already cached.
pub fn observed_lane() -> String {
    format!(
        "lane={}/table-builds={}",
        last_gf256_table_lane(),
        gf256_table_builds()
    )
}
