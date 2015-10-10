//! `miau` is a community IRC bot with a continuous delivery workflow allowing
//! participants to have their changes deployed to the running instance within
//! minutes.

#[macro_use]
extern crate log;
extern crate toml;

pub mod environment;
pub mod logging;

/// entry point!
pub fn main() {
    let env = match environment::load() {
        Ok(env) => env,
        Err(e) => {
            println!("error when loading configuration: {}", e);
            return;
        },
    };

    logging::init().unwrap();

    info!("initialized!");
    info!("host: {:?}", env.conf_str("irc.host"));
    info!("port: {:?}", env.conf_integer("irc.port"));
}
