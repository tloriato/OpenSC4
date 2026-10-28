//! The network-occupant family (docs/specs/network-occupants.md).
//!
//! What is here:
//! - [`identity`]: §1 for all thirteen classes (class IDs, occupant types,
//!   `QueryInterface`, `GetGZCLSID`, `IsOfType`, `IsManagedGlobally`).
//! - [`base`]: the published §2 base owner, with geometry, properties, edge
//!   store, texture cache, path-resource pipeline, lighting and ordered Draw.
//! - [`piece`]: the shared §2 prefix behaviour for `cSC4NetworkOccupant`,
//!   `…WithPreBuiltModel`, `…BridgeOccupant` and `…TunnelOccupant` (flags,
//!   usable message, `Init`/`Shutdown`, path connections, orientation, the
//!   occupied cell). Not the §2.3 record, the geometry or the drawing.
//! - [`power_line`]: §5, `cSC4PowerLineOccupant` slots and its v2 record.
//! - [`power_pole`]: §4 records, shared attachments, controls/quote, geometry,
//!   connection cleanup, model/render ownership and effects (NO33/NO43 applied).
//! - [`pylon`]: §6 record, placement, setters, timer, bitmap/label/billboard
//!   composition and Draw (NO36); retained constructor enum input (NO44).
//! - [`connection`]: §3.6 records, creation/search/lifetimes and statics;
//!   settled geometry/bounds/render/messages (UG5/NO40), bounded NO40-C open.
//!
//! - [`record`], [`xform`]: the §2.3 base record v4-v8 and the transform.
//! - [`underground`]: §3.2 behaviour and the §3.3-§3.5 records.
//! - [`model`]: §10, `cSC4NetworkOccupantModelInstance` (deform, passes, fallback).
//!
//! Not complete: underground and prebuilt owners, and connection's NO40-C
//! runtime observation; see the README's owner audit. Also not here: the
//! type-info tables (§8), and routines other than the
//! ordinary placement-demolition helper in [`placement_demolition`].

pub mod base;
pub mod identity;
pub mod model;
pub mod piece;
pub mod power_line;
pub mod power_pole;
pub mod record;
pub mod surface;
pub mod underground;
pub mod xform;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_model;
#[cfg(test)]
mod tests_record;

pub use identity::{is_of_type, query_interface, NetOccClass, QiAnswer};
pub use model::{NetworkModelInstance, PieceRender};
pub use piece::{world_to_cell, NetworkPiece, PieceHost};
pub use power_line::{PowerLine, PowerLineRecord};
pub use power_pole::PowerLineControl;
pub use record::{read_network_record, write_network_record, LoadEffects, NetworkBaseRecord, Vertex};
pub use underground::{
    read_underground_record, write_underground_record, UndergroundHost, UndergroundModel, UndergroundPiece,
    UndergroundRecord,
};
pub use xform::Xform;

pub mod placement_demolition;

#[cfg(test)]
mod tests_placement_demolition;

#[cfg(test)]
mod tests_power_pole;

pub mod pylon;

#[cfg(test)]
mod tests_pylon;

pub mod connection;

pub mod archive;

#[cfg(test)]
mod tests_connection;
