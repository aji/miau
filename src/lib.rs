//! `miau` is a community IRC bot with a continuous delivery workflow allowing
//! participants to have their changes deployed to the running instance within
//! minutes.

#[macro_use]
extern crate log;
extern crate mio;
extern crate toml;

pub mod core;
pub mod environment;
pub mod event;
pub mod irc;
pub mod logging;
