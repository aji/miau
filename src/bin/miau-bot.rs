//! This is the code for the executable version of the bot. This sets up the
//! necessary things in the core library and starts them off.
//!
//! You won't find anything useful in here, all the interesting things are in
//! the `miau` library.

#[macro_use]
extern crate log;
extern crate miau;
extern crate tokio_core;

use std::process;

fn main() {
    let env = match miau::environment::load() {
        Ok(env) => env,
        Err(e) => {
            println!("there was an error loading configuration: {}", e);
            process::exit(1)
        }
    };

    miau::logging::init(&env).expect("failed to initialize logger");

    let core = tokio_core::reactor::Core::new().expect("failed to create Tokio reactor");

    if let Err(e) = miau::bot::run(env, core) {
        error!("bot exited with an error: {}", e);
        process::exit(1);
    } else {
        info!("bot finished. goodbye!");
    }
}
