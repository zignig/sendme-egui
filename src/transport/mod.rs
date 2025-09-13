// Break the original sendme binary into two halves.
// construct an endpoint at the top level
// This is to fillet the sendme original in half
// and construct endpoints at the top level

use anyhow::Result;
use rand;

use iroh::SecretKey;

mod fetch;
mod offer;

//TODO remove
// mod sendme;

/// Get the secret key or generate a new one.
///
/// Print the secret key to stderr if it was generated, so the user can save it.
pub fn get_or_create_secret(print: bool) -> anyhow::Result<SecretKey> {
    let key = SecretKey::generate(rand::rngs::OsRng);
    Ok(key)
    // match std::env::var("IROH_SECRET") {
    //     Ok(secret) => SecretKey::from_str(&secret).context("invalid secret"),
    //     Err(_) => {
    //         let key = SecretKey::generate(rand::rngs::OsRng);
    //         if print {
    //             let key = hex::encode(key.to_bytes());
    //             eprintln!("using secret key {key}");
    //         }
    //         Ok(key)
    //     }
    // }
}

pub use fetch::receive;
pub use offer::send;
