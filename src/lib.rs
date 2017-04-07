//! `miau` is a community IRC bot with a continuous delivery workflow allowing
//! participants to have their changes deployed to the running instance within
//! minutes.

#[macro_use]
extern crate log;
extern crate toml;
extern crate bytes;
extern crate futures;
extern crate tokio_core;
extern crate tokio_io;

pub mod bot;
pub mod environment;
pub mod irc;
pub mod logging;
