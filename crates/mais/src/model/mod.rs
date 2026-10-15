//! One run, normalised: agents, their model calls, what was in their context, plan readings and the
//! budget's events. Adapters build it; analyses read it.
mod run;
mod usage;

pub use run::*;
pub use usage::*;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a number comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    /// A CLI or the orchestrator reported it.
    Measured,
    /// Arithmetic on measured numbers.
    Derived,
    /// A split by share of characters, a price from a table, or another approximation.
    Estimated,
    /// Not known; shown as unknown, never as zero.
    Unknown,
}

/// A number and where it comes from.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Figure<T> {
    pub value: Option<T>,
    pub provenance: Provenance,
}

impl<T> Default for Figure<T> {
    fn default() -> Self {
        Self::unknown()
    }
}

impl<T> Figure<T> {
    pub fn measured(v: T) -> Self {
        Self { value: Some(v), provenance: Provenance::Measured }
    }
    pub fn derived(v: T) -> Self {
        Self { value: Some(v), provenance: Provenance::Derived }
    }
    pub fn estimated(v: T) -> Self {
        Self { value: Some(v), provenance: Provenance::Estimated }
    }
    pub fn unknown() -> Self {
        Self { value: None, provenance: Provenance::Unknown }
    }
}
