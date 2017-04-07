//! The main bot entry point.

// This file is terrible and extremely in flux. Please don't rely on anything
// in it remaining the same in the short future!

use std::io;
use std::mem;
use std::net::ToSocketAddrs;
use std::str;
use std::thread;
use std::time;

use bytes::BufMut;
use bytes::BytesMut;

use futures::Future;
use futures::Poll;
use futures::Async;
use futures::Stream;
use futures::Sink;
use futures::AsyncSink;

use tokio_core::reactor::Core;
use tokio_core::reactor::Handle;
use tokio_core::net::TcpStream;
use tokio_core::net::TcpStreamNew;
use tokio_io::AsyncRead;
use tokio_io::AsyncWrite;
use tokio_io::codec::Decoder;
use tokio_io::codec::Encoder;
use tokio_io::codec::Framed;

use toml;

use environment::Env;
use irc;

pub struct Bot<S> {
    env: Env,
    _handle: Handle,
    sock: Framed<S, IrcCodec>,
    sock_state: SockState,
}

enum SockState {
    Empty,
    Receiving,
    Sending(Vec<String>),
    PollComplete,
    Finish,
}

impl<S: AsyncRead + AsyncWrite + Sized> Bot<S> {
    fn new(env: Env, handle: Handle, sock: S) -> Bot<S> {
        let mut bot = Bot {
            env: env,
            _handle: handle,
            sock: sock.framed(IrcCodec),
            sock_state: SockState::Receiving,
        };
        bot.send_registration();
        bot
    }

    fn canonical_nick(&self) -> &str {
        self.env.conf_str("irc.nick").unwrap_or("miau")
    }

    fn send(&mut self, line: String) {
        let mut lines = match mem::replace(&mut self.sock_state, SockState::Empty) {
            SockState::Sending(lines) => lines,
            _ => Vec::with_capacity(1),
        };
        lines.push(line);
        self.sock_state = SockState::Sending(lines);
    }

    fn send_registration(&mut self) {
        let nick_line = format!("NICK {}", self.canonical_nick());
        let user_line = format!("USER miau * * :{}", env!("CARGO_PKG_HOMEPAGE"));
        self.send(nick_line);
        self.send(user_line);
    }

    fn handle_line(&mut self, line: String) {
        let m = match irc::Message::parse(&line[..]) {
            Ok(m) => m,
            Err(e) => { error!("bad line: {}", e); return; },
        };

        match m.verb {
            "001" => {
                // TODO: this is EXTREMELY UGLY, please fix it.
                let dfl = vec![toml::value::Value::String("#miau-dev".to_owned())];
                let chans = self.env.conf_array("irc.channels").unwrap_or(&dfl).to_owned();
                for chan in chans.iter() {
                    let c = match chan {
                        &toml::value::Value::String(ref c) => c,
                        _ => {
                            warn!("{} is not a string! skipping", chan);
                            continue;
                        }
                    };
                    self.send(format!("JOIN {}", c));
                }
            },

            "PRIVMSG" if m.args[0].starts_with("#") => {
                if m.args[1] == format!("{}: version", self.canonical_nick()) {
                    self.send(format!("PRIVMSG {} :i am {} v{}", m.args[0],
                        env!("CARGO_PKG_NAME"),
                        env!("CARGO_PKG_VERSION")));
                }
            },

            "PING" => self.send(format!("PONG :{}", m.args[0])),

            _ => {}
        }
    }
}

impl<S: AsyncRead + AsyncWrite> Future for Bot<S> {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        loop {
            match mem::replace(&mut self.sock_state, SockState::Empty) {
                SockState::Empty => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        "cannot poll Bot in SockState::Empty"
                    ))
                },

                SockState::Receiving => {
                    match self.sock.poll() {
                        Ok(Async::Ready(Some(s))) => {
                            self.sock_state = SockState::Receiving;
                            self.handle_line(s);
                        },
                        Ok(Async::Ready(None)) => {
                            debug!("EOF!");
                            self.sock_state = SockState::Finish;
                        },
                        Ok(Async::NotReady) => {
                            self.sock_state = SockState::Receiving;
                            return Ok(Async::NotReady);
                        },
                        Err(e) => {
                            return Err(e);
                        },
                    }
                },

                SockState::Sending(ref lines) if lines.len() == 0 => {
                    self.sock_state = SockState::PollComplete;
                },

                SockState::Sending(mut lines) => {
                    let line = lines.remove(0); // XXX: horribly inefficient

                    match self.sock.start_send(line) {
                        Ok(AsyncSink::Ready) => {
                            self.sock_state = SockState::Sending(lines);
                        },
                        Ok(AsyncSink::NotReady(line)) => {
                            lines.insert(0, line); // XXX: also horribly inefficient
                            self.sock_state = SockState::Sending(lines);
                            return Ok(Async::NotReady);
                        },
                        Err(e) => {
                            return Err(e);
                        },
                    }
                },

                SockState::PollComplete => {
                    match self.sock.poll_complete() {
                        Ok(Async::Ready(())) => {
                            self.sock_state = SockState::Receiving;
                        },
                        Ok(Async::NotReady) => {
                            self.sock_state = SockState::PollComplete;
                            return Ok(Async::NotReady);
                        },
                        Err(e) => {
                            return Err(e);
                        },
                    }
                },

                SockState::Finish => {
                    return Ok(Async::Ready(()));
                },
            }
        }
    }
}

struct IrcCodec;

impl Decoder for IrcCodec {
    type Item = String;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<String>, io::Error> {
        loop {
            let n_loc = src.iter().position(|b| *b == b'\n');
            let r_loc = src.iter().position(|b| *b == b'\r');

            let (nl_start, nl_size) = match n_loc {
                None => return Ok(None), // no \n
                Some(i) => match r_loc {
                    Some(j) if j + 1 == i => (j, 2), // \r\n
                    _ => (i, 1), // \n
                },
            };

            let line = src.split_to(nl_start);
            src.split_to(nl_size);

            if line.len() != 0 {
                return match str::from_utf8(&line[..]) {
                    Ok(s) => {
                        debug!(" --> {}", s);
                        Ok(Some(s.to_string()))
                    },
                    Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
                };
            }
        }
    }
}

impl Encoder for IrcCodec {
    type Item = String;
    type Error = io::Error;

    fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<(), io::Error> {
        debug!(" <-- {}", item);
        dst.put(item);
        dst.put(b'\r');
        dst.put(b'\n');
        Ok(())
    }
}

pub fn run(env: Env, mut reactor: Core) -> io::Result<()> {
    let handle = reactor.handle();

    let wait = env.conf_integer_or("bot.start_delay", 30) as u64;
    info!("sleeping for {} seconds before attempting connection", wait);
    thread::sleep(time::Duration::new(wait, 0));

    let connect = try!(start_connect(env.clone(), reactor.handle()));

    let bot = connect.and_then(move |sock| {
        info!("connected! starting the bot...");
        Bot::new(env, handle, sock)
    });

    reactor.run(bot)
}

fn start_connect(env: Env, handle: Handle) -> io::Result<TcpStreamNew> {
    let host = match env.conf_str("irc.host") {
        Some(host) => host,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "config value irc.host wrong type or missing"
            ));
        },
    };

    let port = match env.conf_integer("irc.port") {
        Some(port) => port as u16,
        None => {
            debug!("using default port of 6667");
            6667
        },
    };

    debug!("looking up {}:{}", host, port);

    let addr = match try!((host, port).to_socket_addrs()).nth(0) {
        Some(addr) => addr,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "no usable adresses!"
            ));
        }
    };

    info!("starting connect to {}", addr);

    Ok(TcpStream::connect(&addr, &handle))
}
