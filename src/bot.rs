//! The main bot entry point.

use std::collections::VecDeque;
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
use futures::task;

use tokio_core::reactor::Core;
use tokio_core::reactor::Handle;
use tokio_core::net::TcpStream;
use tokio_core::net::TcpStreamNew;
use tokio_io::AsyncRead;
use tokio_io::AsyncWrite;
use tokio_io::codec::Decoder;
use tokio_io::codec::Encoder;
use tokio_io::codec::Framed;

use environment::Env;
use irc;
use network;

pub struct Bot<S> {
    _env: Env,
    _handle: Handle,
    sock: Sock<S>,
    bot_state: BotState,
    net: network::Network,
}

enum BotState {
    Invalid,
    Start,
    Receiving,
    Finish,
}

impl<S: AsyncRead + AsyncWrite + Sized> Bot<S> {
    fn new(env: Env, handle: Handle, raw_sock: S) -> Bot<S> {
        let mut sock = Sock::new(raw_sock);
        let net = network::Network::register(env.clone(), &mut sock);

        Bot {
            _env: env,
            _handle: handle,
            sock: sock,
            bot_state: BotState::Start,
            net: net,
        }
    }
}

impl<S> Bot<S> {
    fn handle_line(&mut self, line: String) {
        match irc::Message::parse(&line[..]) {
            Ok(m) => self.net.handle_message(&mut self.sock, m),
            Err(e) => error!("could not parse IRC message: {}", e),
        };
    }
}

impl<S: AsyncRead + AsyncWrite> Future for Bot<S> {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<(), io::Error> {
        loop {
            match mem::replace(&mut self.bot_state, BotState::Invalid) {
                BotState::Invalid => {
                    warn!("bot ended up in invalid state!");
                    self.bot_state = BotState::Start;
                },

                BotState::Start => {
                    self.bot_state = BotState::Receiving;
                },

                BotState::Receiving => {
                    match self.sock.poll() {
                        Ok(Async::Ready(Some(s))) => {
                            self.bot_state = BotState::Start;
                            self.handle_line(s);
                        },
                        Ok(Async::Ready(None)) => {
                            info!("end of input. finishing");
                            self.bot_state = BotState::Finish;
                        },
                        Ok(Async::NotReady) => {
                            self.bot_state = BotState::Start;
                            return Ok(Async::NotReady);
                        },
                        Err(e) => {
                            return Err(e);
                        },
                    }
                },

                BotState::Finish => {
                    self.bot_state = BotState::Finish;
                    return Ok(Async::Ready(()));
                },
            }
        }
    }
}

struct Sock<S> {
    sock: Framed<S, IrcCodec>,
    sock_state: SockState,
    out_buf: VecDeque<String>,
    parked: Option<task::Task>,
}

enum SockState {
    Invalid,
    Start,
    Receiving,
    Sending,
    PollComplete,
    Finish,
}

impl<S: AsyncRead + AsyncWrite + Sized> Sock<S> {
    fn new(sock: S) -> Sock<S> {
        Sock {
            sock: sock.framed(IrcCodec),
            sock_state: SockState::Start,
            out_buf: VecDeque::new(),
            parked: None,
        }
    }
}

impl<S> Sock<S> {
    fn send(&mut self, line: String) {
        self.out_buf.push_back(line);
        self.parked.take().map(|t| t.unpark());
    }
}

impl<S> network::Output for Sock<S> {
    fn send(&mut self, line: String) {
        Sock::send(self, line);
    }
}

impl<S: AsyncRead + AsyncWrite> Stream for Sock<S> {
    type Item = String;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<String>, io::Error> {
        loop {
            match mem::replace(&mut self.sock_state, SockState::Invalid) {
                SockState::Invalid => {
                    warn!("socket ended up in invalid state!");
                    self.sock_state = SockState::Start;
                },

                SockState::Start => {
                    if self.out_buf.is_empty() {
                        self.sock_state = SockState::Receiving;
                    } else {
                        self.sock_state = SockState::Sending;
                    }
                },

                SockState::Receiving => {
                    match self.sock.poll() {
                        Ok(Async::Ready(None)) => {
                            self.sock_state = SockState::Finish;
                        },
                        Ok(Async::Ready(Some(s))) => {
                            self.sock_state = SockState::Start;
                            return Ok(Async::Ready(Some(s)));
                        },
                        Ok(Async::NotReady) => {
                            self.sock_state = SockState::Start;
                            self.parked = Some(task::park());
                            return Ok(Async::NotReady);
                        },
                        Err(e) => {
                            return Err(e);
                        },
                    }
                },

                SockState::Sending => {
                    if let Some(line) = self.out_buf.pop_front() {
                        match self.sock.start_send(line) {
                            Ok(AsyncSink::Ready) => {
                                self.sock_state = SockState::Sending;
                            },
                            Ok(AsyncSink::NotReady(line)) => {
                                self.out_buf.push_front(line);
                                self.sock_state = SockState::Sending;
                                self.parked = Some(task::park());
                                return Ok(Async::NotReady);
                            },
                            Err(e) => {
                                return Err(e);
                            },
                        }
                    } else {
                        self.sock_state = SockState::PollComplete;
                    }
                },

                SockState::PollComplete => {
                    match self.sock.poll_complete() {
                        Ok(Async::Ready(())) => {
                            self.sock_state = SockState::Start;
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
                    self.sock_state = SockState::Finish;
                    return Ok(Async::Ready(None));
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
                    Err(e) => {
                        warn!("could not parse bytes as utf8: {:?}: {}", line, e);
                        Ok(None)
                    }
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
