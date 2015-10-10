#[macro_use]
extern crate log;
extern crate miau;

use miau::*;

pub fn main() {
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
