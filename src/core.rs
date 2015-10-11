//! The main bot entry point.

// This file is terrible and extremely in flux. Please don't rely on anything
// in it remaining the same in the short future!

use mio;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::net::ToSocketAddrs;

use environment::Env;
use irc;

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
        let mut hdl = try!(BotHandler::new(self));

        try!(hdl.connect(&mut evt));
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

        if !self.registered {
            let nick = self.env.conf_str("irc.nick").unwrap_or("miau");
            sock.write_fmt(format_args!("NICK {}\r\n", nick)).unwrap();
            sock.write_fmt(format_args!("USER a a a a\r\n")).unwrap();
            self.registered = true;
        }

        match msg.verb {
            b"001" => {
                sock.write_fmt(format_args!("JOIN #miau-dev\r\n")).unwrap();
            },
            b"PING" => {
                sock.write_fmt(format_args!("PONG :{}\r\n",
                    String::from_utf8_lossy(msg.args[0]))).unwrap();
            },
            _ => { }
        }
    }

    fn on_end_of_data(&mut self) {
        trace!("end of stream");
    }

    fn on_incoming_data(&mut self, data: &[u8], sock: &mut mio::tcp::TcpStream) {
        trace!("{} bytes incoming", data.len());
        for byte in data {
            match *byte {
                b'\r' => { },
                b'\n' => { self.on_line(sock); self.line.clear(); }
                _     => { self.line.push(*byte); }
            }
        }
    }
}

// this doesn't need to have any special meaning right now, it just has to be
// unique. I'm using 123 because 0 or 1 looks like a special value.
const CONN_TOKEN: usize = 123;

const READ_BUF_SIZE: usize = 2048;

struct BotHandler<'a> {
    bot: &'a mut Bot<'a>,

    conntok: Option<mio::Token>,
    conn: Option<mio::tcp::TcpStream>,
}

impl<'a> BotHandler<'a> {
    fn new(bot: &'a mut Bot<'a>) -> io::Result<BotHandler<'a>> {
        Ok(BotHandler {
            bot: bot,

            conntok: None,
            conn: None,
        })
    }

    fn connect(&mut self, evt: &mut mio::EventLoop<Self>)
    -> io::Result<()> {
        let host = match self.bot.env.conf_str("irc.host") {
            Some(host) => host,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "config value irc.host wrong type or missing"
                ));
            },
        };

        let port = match self.bot.env.conf_integer("irc.port") {
            Some(port) => port as u16,
            None => {
                debug!("using default port of 6667");
                6667
            },
        };

        debug!("resolving {}:{}", host, port);
        let addr = match try!((host, port).to_socket_addrs()).nth(0) {
            Some(addr) => addr,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "could not resolve the address!"
                ));
            },
        };
        trace!("resolves to {}", addr);

        info!("connecting to {}:{}", host, port);
        let sock = try!(mio::tcp::TcpStream::connect(&addr));
        let token = mio::Token(CONN_TOKEN);

        // these have to succeed at the same time or weirdness will happen. in
        // the future both of these values should be encapsulated in the same
        // struct.
        self.conntok = Some(token);
        self.conn = Some(sock);

        if let Some(sockref) = self.conn.as_ref() {
            trace!("registering new socket with the event loop");
            try!(evt.register_opt(
                sockref, token,
                mio::EventSet::readable() |
                mio::EventSet::error() |
                mio::EventSet::hup(),
                mio::PollOpt::edge()
            ));
            trace!("registered!");
        }

        debug!("successful connection!");
        Ok(())
    }

    fn sock(&mut self) -> Option<&mut mio::tcp::TcpStream> {
        self.conn.as_mut()
    }
}

impl<'a> mio::Handler for BotHandler<'a> {
    type Timeout = ();
    type Message = ();

    fn ready(
        &mut self,
        evt: &mut mio::EventLoop<Self>,
        tok: mio::Token,
        _events: mio::EventSet
    ) {
        trace!("{:?} becomes readable", tok);

        match self.conntok {
            Some(conntok) => if conntok != tok {
                debug!("ready() called with a token we don't care about?");
                return;
            },
            None => {
                debug!("ready() called while we're not connected");
                return;
            },
        }

        let mut buf: [u8; READ_BUF_SIZE] = unsafe { mem::uninitialized() };
        let mut stop = true;

        if let Some(ref mut conn) = self.conn {
            match conn.read(&mut buf[..]) {
                Ok(0) => self.bot.on_end_of_data(),
                Ok(size) => {
                    self.bot.on_incoming_data(&buf[..size], conn);
                    stop = false;
                },
                Err(e) => error!("error when reading: {}", e),
            }
        } else {
            error!("conn is None, but we are registered!");
        }

        if stop {
            evt.shutdown();
        }
    }
}
