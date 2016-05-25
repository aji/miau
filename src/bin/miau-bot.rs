//! This is the code for the executable version of the bot. This sets up the
//! necessary things in the core library and starts them off.
//!
//! You won't find anything useful in here, all the interesting things are in
//! the `miau` library.

#[macro_use]
extern crate log;
extern crate miau;

use std::process;
use std::thread;
use std::time;

use miau::*;

fn main() {
    let env = match environment::load() {
        Ok(env) => env,
        Err(e) => {
            println!("error when loading configuration: {}", e);
            process::exit(1)
        },
    };

    logging::init(&env).unwrap();

    let mut bot = match core::Bot::new(&env) {
        Ok(bot) => bot,
        Err(_) => {
            error!("couldn't create the bot!");
            process::exit(1)
        },
    };

    let wait = env.conf_integer_or("bot.start_delay", 30) as u64;
    info!("sleeping for {} seconds before attempting connection", wait);
    thread::sleep(time::Duration::new(wait, 0));

    match bot.run() {
        Ok(_) => {
            info!("bot finished. bye!");
        },
        Err(e) => {
            error!("an error occurred: {}", e);
            process::exit(1)
        },
    }
}
