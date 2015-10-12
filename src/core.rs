//! The main bot entry point.

// This file is terrible and extremely in flux. Please don't rely on anything
// in it remaining the same in the short future!

use mio;
use std::io;
use std::io::prelude::*;
use toml;

use environment::Env;
use irc;
use net;

pub struct Bot<'a> {
    env: &'a Env,
    line: Vec<u8>,
    registered: bool,
}

impl<'a> Bot<'a> {
    pub fn new(env: &Env) -> Result<Bot, ()> {
        Ok(Bot {
            env: env,
            line: Vec::new(),
            registered: false,
        })
    }

    pub fn run(&'a mut self) -> io::Result<()> {
        let mut evt = try!(mio::EventLoop::new());
        let mut hdl = try!(net::NetHandler::new(self));

        let host = match self.env.conf_str("irc.host") {
            Some(host) => host,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "config value irc.host wrong type or missing"
                ));
            },
        };

        let port = match self.env.conf_integer("irc.port") {
            Some(port) => port as u16,
            None => {
                debug!("using default port of 6667");
                6667
            },
        };

        debug!("starting connection to {}:{}", host, port);
        try!(hdl.connect(&mut evt, (host, port)));
        try!(evt.run(&mut hdl));

        Ok(())
    }

    fn on_line(&mut self, sock: &mut mio::tcp::TcpStream) {
        debug!(" -> {}", String::from_utf8_lossy(&self.line[..]));

        let msg = match irc::Message::parse(&self.line[..]) {
            Ok(msg) => msg,
            Err(e) => {
                warn!("ignoring message: parse failed: {}", e);
                return;
            }
        };

        trace!("parsed output: {:?}", msg);

        match msg.verb {
            b"001" => {
                let dfl = [toml::Value::String("#miau-dev".to_owned())];
                let chans = self
                    .env.conf_slice("irc.channels")
                    .unwrap_or(&dfl[..]);
                for chan in chans {
                    let c = match chan {
                        &toml::Value::String(ref c) => c,
                        _ => {
                            warn!("{} is not a string! skipping", chan);
                            continue;
                        }
                    };
                    sock.write_fmt(format_args!("JOIN {}\r\n", c)).unwrap();
                }
            },
            b"PING" => {
                sock.write_fmt(format_args!("PONG :{}\r\n",
                    String::from_utf8_lossy(msg.args[0]))).unwrap();
            },
            _ => { }
        }
    }
}

impl<'a> net::NetDelegate for Bot<'a> {
    fn on_end_of_data(
        &mut self,
        evt: &mut mio::EventLoop<net::NetHandler<Self>>,
        ctx: &mut net::NetContext
    ) {
        trace!("end of stream");
    }

    fn on_incoming_data(
        &mut self,
        evt: &mut mio::EventLoop<net::NetHandler<Self>>,
        ctx: &mut net::NetContext,
        data: &[u8]
    ) {
        trace!("{} bytes incoming", data.len());
        for byte in data {
            match *byte {
                b'\r' => { },
                b'\n' => { self.on_line(ctx.sock()); self.line.clear(); }
                _     => { self.line.push(*byte); }
            }
        }
    }

    fn on_writable(
        &mut self,
        evt: &mut mio::EventLoop<net::NetHandler<Self>>,
        ctx: &mut net::NetContext
    ) {
        if self.registered {
            return;
        }

        debug!("became writable. registering");

        let nick = self.env.conf_str("irc.nick").unwrap_or("miau");
        ctx.sock().write_fmt(format_args!("NICK {}\r\n", nick)).unwrap();
        ctx.sock().write_fmt(format_args!("USER a a a a\r\n")).unwrap();

        self.registered = true;

        ctx.reregister_read_only(evt);
    }
}
