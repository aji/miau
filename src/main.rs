//! `miau` is a community IRC bot with a continuous delivery workflow allowing
//! participants to have their changes deployed to the running instance within
//! minutes.

#[macro_use]
extern crate log;

pub mod logging;

/// entry point!
pub fn main() {
    logging::init().unwrap();

    info!("Hello, world!");
}
