//! Environment and configuration management.
//!
//! "Environment" is just another word for configuration, but the idea is that
//! everything that could affect the way the bot makes configuration-based
//! decisions should be encapsulated in the structures found in this class, most
//! notably [`Env`](struct.Env.html).
//!
//! Since the environment must be loaded before anything else can happen,
//! the [`load`](fn.load.html) takes no arguments. Anything that could affect
//! the way the environment is loaded should itself be part of the environment,
//! so the logic for detecting those things should live in this module.

// There's a sort of Catch-22 when dealing with configuration, in that we can't
// actually put any information about the configuration itself into the
// configuration files. So a very minimal amount of configuration information
// has to be passed in from the system, such as in environment variables and on
// the command line. Fortunately, this special case is restricted to this file,
// so configuration management elsewhere in the codebase should restrict itself
// to using the functionality provided by this module.

use std::convert::From;
use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::rc::Rc;
use toml;

const CONFIG_ENV:  &'static str = "MIAU_CONFIG";
const OVERLAY_ENV: &'static str = "MIAU_OVERLAY";

const DEFAULT_CONFIG:  &'static str = "config/miau-prod.toml";
const DEFAULT_OVERLAY: &'static str = "config/miau-dev.toml";

pub type Env = Rc<EnvInner>;

/// The main `struct` for accessing the bot's environment, including
/// configuration loaded from files, the command line, and environment
/// variables. Any static user configuration should be retrieved from an
/// instance of this struct.
pub struct EnvInner {
    config:   toml::Value,
    overlay:  toml::Value,
}

impl EnvInner {
    /// Fetches the given configuration value, if it exists. Refer to the
    /// `lookup` documentation for the meaning of the "path"
    /// argument.
    pub fn conf<'a>(&'a self, path: &'a str) -> Option<&toml::Value> {
        lookup(&self.overlay, path).or_else(|| lookup(&self.config, path))
    }

    /// Fetches the given configuration value as a string slice, if it exists.
    pub fn conf_str<'a>(&'a self, path: &'a str) -> Option<&str> {
        self.conf(path).and_then(|v| v.as_str())
    }

    /// Fetches the given configuration value as an integer, if it exists.
    pub fn conf_integer<'a>(&'a self, path: &'a str) -> Option<i64> {
        self.conf(path).and_then(|v| v.as_integer())
    }

    /// Fetches the given configuration value as an integer, or the default value,
    /// if it doesn't exist, and prints a warning.
    pub fn conf_integer_or<'a>(&'a self, path: &'a str, or: i64) -> i64 {
        self.conf_integer(path).unwrap_or_else(|| { warn!("{} defaulting to {}", path, or); or })
    }

    /// Fetches the given configuration value as a floating point value, if it
    /// exists.
    pub fn conf_float<'a>(&'a self, path: &'a str) -> Option<f64> {
        self.conf(path).and_then(|v| v.as_float())
    }

    /// Fetches the given configuration value as a boolean, if it exists.
    pub fn conf_bool<'a>(&'a self, path: &'a str) -> Option<bool> {
        self.conf(path).and_then(|v| v.as_bool())
    }

    /// Fetches the given configuration value as an array of TOML values, if it
    /// exists.
    pub fn conf_array<'a>(&'a self, path: &'a str) -> Option<&Vec<toml::Value>> {
        self.conf(path).and_then(|v| v.as_array())
    }

    /// Fetches the given configuration value as a TOML table, if it exists.
    pub fn conf_table<'a>(&'a self, path: &'a str) -> Option<&toml::value::Table> {
        self.conf(path).and_then(|v| v.as_table())
    }
}

/// Look up a value. Splits on periods, descends that way.
pub fn lookup<'t>(x: &'t toml::Value, path: &str) -> Option<&'t toml::Value> {
    path.split('.').fold(Some(x), |x, p| x.and_then(|x| lookup_one(x, p)))
}

fn lookup_one<'t>(x: &'t toml::Value, item: &str) -> Option<&'t toml::Value> {
    if x.is_array() {
        item.parse::<usize>().ok().and_then(|i| x.get(i))
    } else {
        x.get(item)
    }
}

/// Used to signal that there was an error loading the environment.
pub enum Error {
    IO(io::Error),
    TOML(String, toml::de::Error),
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error { Error::IO(err) }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IO(ref err) => write!(f, "{}", err),
            Error::TOML(ref path, ref err) => write!(f, "{}: {}", path, err),
        }
    }
}

fn read_toml(path: String) -> Result<toml::Value, Error> {
    let mut file = try!(fs::File::open(&path[..]));

    let mut data = String::new();
    try!(file.read_to_string(&mut data));

    toml::from_str::<toml::Value>(&data[..]).map_err(|e| Error::TOML(path, e))
}

/// Loads the bot's environment.
///
/// This checks two environment variables to determine the paths to load
/// configuration files from, `MIAU_CONFIG` and `MIAU_OVERLAY`, corresponding to
/// the base and overlay configuration files respectively. The files are parsed
/// as TOML.
pub fn load() -> Result<Env, Error> {
    let config = match env::var(CONFIG_ENV) {
        Ok(conf)  => conf,
        Err(_)    => DEFAULT_CONFIG.to_string(),
    };

    let overlay = match env::var(OVERLAY_ENV) {
        Ok(over) => over,
        Err(_) => {
            println!("warning: {} defaults to {}", OVERLAY_ENV, DEFAULT_OVERLAY);
            DEFAULT_OVERLAY.to_string()
        }
    };

    let env = EnvInner {
        config:  try!(read_toml(config)),
        overlay: try!(read_toml(overlay)),
    };

    Ok(Rc::new(env))
}
