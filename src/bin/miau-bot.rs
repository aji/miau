//! This is the code for the executable version of the bot. This sets up the
//! necessary things in the core library and starts them off.
//!
//! You won't find anything useful in here, all the interesting things are in
//! the `miau` library.

#[macro_use]
extern crate log;
extern crate miau;

use miau::*;

fn main() {
    let env = match environment::load() {
        Ok(env) => env,
        Err(e) => {
            println!("error when loading configuration: {}", e);
            return;
        },
    };

    logging::init(&env).unwrap();

    info!("initialized!");
}
