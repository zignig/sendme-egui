// Break the original sendme binary into two halves.
// construct an endpoint at the top level

use crate::comms::MessageOut;
use anyhow::Result;
use anyhow::anyhow;


mod fetch;
mod offer;

pub use offer::send;
pub use fetch::receive;

