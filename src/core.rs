//! The main bot entry point.

// This file is terrible and extremely in flux. Please don't rely on anything
// in it remaining the same in the short future!

use std::io;
use toml;

use environment::Env;
use event;
use irc;

pub struct Bot<'a> {
    env: &'a Env,
    line: Vec<u8>,
}

impl<'a> Bot<'a> {
    pub fn new(env: &Env) -> Result<Bot, ()> {
        Ok(Bot {
            env: env,
            line: Vec::new(),
        })
    }

    pub fn run(&mut self) -> io::Result<()> {
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

        let mut reactor = event::Reactor::new();
        let me = reactor.add_handler(self);
        let conn = try!(reactor.connect((host, port), me));
        let nick = self.env.conf_str("irc.nick").unwrap_or("miau");
        reactor.queue_send(conn, b"NICK ");
        reactor.queue_send(conn, nick.as_bytes());
        reactor.queue_send(conn, b"\r\n");
        reactor.queue_send(conn, b"USER a a a a\r\n");

        reactor.run();

        Ok(())
    }

    fn on_line<'b>(&mut self, ev: &mut event::EventedCtl<'b>) {
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
                    ev.queue_send(b"JOIN ");
                    ev.queue_send(c.as_bytes());
                    ev.queue_send(b"\r\n");
                }
            },
            b"PING" => {
                ev.queue_send(b"PONG :");
                ev.queue_send(msg.args[0]);
                ev.queue_send(b"\r\n");
            },
            _ => { }
        }
    }
}

impl<'a> event::Handler for Bot<'a> {
    fn on_end_of_input<'b>(
        &mut self,
        _ev: &mut event::EventedCtl<'b>,
    ) {
        trace!("end of stream");
    }

    fn on_incoming_data<'b>(
        &mut self,
        ev: &mut event::EventedCtl<'b>,
        data: &[u8]
    ) {
        trace!("{} bytes incoming", data.len());
        for byte in data {
            match *byte {
                b'\r' => { },
                b'\n' => { self.on_line(ev); self.line.clear(); }
                _     => { self.line.push(*byte); }
            }
        }
    }
}
