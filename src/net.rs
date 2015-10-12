use mio;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::net::ToSocketAddrs;

// the token doesn't have any special meaning, it just has to be unique.
// 0 or 1 look like special values, so 6 it is.
const CONN_TOKEN: usize = 6;
const READ_BUF_SIZE: usize = 4096;

pub trait NetDelegate: Sized {
    fn on_end_of_data(
        &mut self,
        evt: &mut mio::EventLoop<NetHandler<Self>>,
        ctx: &mut NetContext,
    );

    fn on_incoming_data(
        &mut self,
        evt: &mut mio::EventLoop<NetHandler<Self>>,
        ctx: &mut NetContext,
        data: &[u8],
    );

    fn on_writable(
        &mut self,
        evt: &mut mio::EventLoop<NetHandler<Self>>,
        ctx: &mut NetContext,
    );
}

pub struct NetContext<'a> {
    sock: &'a mut mio::tcp::TcpStream
}

impl<'a> NetContext<'a> {
    pub fn sock(&mut self) -> &mut mio::tcp::TcpStream {
        self.sock
    }

    pub fn reregister_read_only<H: mio::Handler>(
        &mut self,
        evt: &mut mio::EventLoop<H>,
    ) {
        evt.reregister(
            self.sock, mio::Token(CONN_TOKEN),
            mio::EventSet::readable() |
            mio::EventSet::error() |
            mio::EventSet::hup(),
            mio::PollOpt::edge()
        ).unwrap();
    }
}

struct Connection {
    tok: mio::Token,
    conn: mio::tcp::TcpStream,
}

pub struct NetHandler<'a, D: NetDelegate + 'a> {
    delegate: &'a mut D,
    conn: Option<Connection>,
}

impl<'a, D> NetHandler<'a, D> where D: NetDelegate {
    pub fn new(delegate: &mut D) -> io::Result<NetHandler<D>> {
        Ok(NetHandler { delegate: delegate, conn: None })
    }

    pub fn connect<A: ToSocketAddrs>(
        &mut self,
        evt: &mut mio::EventLoop<Self>,
        addr: A
    ) -> io::Result<()> {
        debug!("resolving address...");
        let addr = match try!(addr.to_socket_addrs()).nth(0) {
            Some(addr) => addr,
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "could not resolve the address!"
                ));
            },
        };
        trace!("resolves to {}", addr);

        info!("starting connection...");
        let sock = try!(mio::tcp::TcpStream::connect(&addr));
        let token = mio::Token(CONN_TOKEN);

        self.conn = Some(Connection {
            tok: token,
            conn: sock,
        });

        if let Some(connref) = self.conn.as_ref() {
            trace!("registering new socket with the event loop");
            try!(evt.register_opt(
                &connref.conn, token,
                mio::EventSet::readable() |
                mio::EventSet::writable() |
                mio::EventSet::error() |
                mio::EventSet::hup(),
                mio::PollOpt::edge()
            ));
            trace!("registered!");
        }

        debug!("successfully initiated connection!");
        Ok(())
    }
}

impl<'a, D> mio::Handler for NetHandler<'a, D> where D: NetDelegate {
    type Timeout = ();
    type Message = ();

    fn ready(
        &mut self,
        evt: &mut mio::EventLoop<Self>,
        tok: mio::Token,
        events: mio::EventSet
    ) {
        let mut buf: [u8; READ_BUF_SIZE] = unsafe { mem::uninitialized() };

        let conn = match self.conn.as_mut() {
            Some(conn) => if conn.tok != tok {
                debug!("ready() called with a token we don't care about?");
                return;
            } else {
                &mut conn.conn
            },
            None => {
                debug!("ready() called before we're connected");
                return;
            },
        };

        let mut ctx = NetContext { sock: conn };

        if events.is_readable() {
            let read = ctx.sock().read(&mut buf[..]);

            let stop = match read {
                Ok(0) => {
                    self.delegate.on_end_of_data(evt, &mut ctx);
                    true
                },
                Ok(size) => {
                    self.delegate.on_incoming_data(evt, &mut ctx, &buf[..size]);
                    false
                },
                Err(e) => {
                    error!("error when reading: {}", e);
                    true
                },
            };

            if stop {
                evt.shutdown();
            }
        }

        if events.is_writable() {
            self.delegate.on_writable(evt, &mut ctx);
        }
    }
}
