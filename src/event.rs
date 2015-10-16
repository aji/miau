//! Ergonomic (but slightly slower) event loop functionality on top of `mio`
//!
//! The extra slowness is due to the hearty use of trait objects. Trait objects
//! are like Java interfaces, and only slightly slower than typical use of
//! traits in Rust due to all function calls on the object being dynamic, rather
//! than static. However, as miau is network-bound, this tradeoff is acceptable
//! for the immense ergonomic benefit it affords.

// NOTE: CONTACT AJI BEFORE WORKING IN HERE, IT'S A DEFINITE WORK IN PROGRESS.

// TODO: less panic/unwrap, more io::Result<>

use mio;
use std::collections::HashMap;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::net::ToSocketAddrs;

const RECV_BUF_SIZE: usize = 2048;

pub type HandlerName = usize;
pub type EventedName = usize;

pub struct Reactor<'h> {
    next_handler: HandlerName,
    next_evented: EventedName,
    handlers: HashMap<HandlerName, &'h mut Handler>,
    evthdlrs: HashMap<EventedName, HandlerName>,
    eventeds: HashMap<EventedName, Box<Evented>>,
}

pub struct ReactorCtl<'a> {
    next_evented: &'a mut EventedName,
    new_eventeds: Vec<(EventedName, HandlerName, Box<Evented>)>,
    queued_sends: Vec<(EventedName, Vec<u8>)>,
}

pub trait Evented {
    fn register<'h>(
        &self,
        event_loop: &mut mio::EventLoop<Reactor<'h>>,
        tok: mio::Token
    );
    fn reregister<'h>(
        &self,
        event_loop: &mut mio::EventLoop<Reactor<'h>>,
        tok: mio::Token
    );

    fn queue_send(&mut self, data: &[u8]);

    fn recv_ready<'a>(
        &mut self,
        nm: EventedName,
        h: &mut Handler,
        rc: &mut ReactorCtl<'a>
    );
    fn send_ready<'a>(
        &mut self,
        nm: EventedName,
        h: &mut Handler,
        rc: &mut ReactorCtl<'a>
    );
}

pub trait EventedCtl<'a> {
    fn name(&self) -> EventedName;
    fn reactor(&mut self) -> &mut ReactorCtl<'a>;

    fn queue_send(&mut self, data: &[u8]);

    fn stop_recv(&mut self);
    fn stop_send(&mut self);
}

pub trait Handler {
    fn on_end_of_input<'a>(&mut self, ev: &mut EventedCtl<'a>);
    fn on_incoming_data<'a>(&mut self, ev: &mut EventedCtl<'a>, data: &[u8]);
}

pub struct EventedSocket {
    sock: mio::tcp::TcpStream,
    outbuf: Vec<u8>,
}

impl<'h> Reactor<'h> {
    pub fn new() -> Reactor<'h> {
        // the 'nexts' could be anything, but I like 4
        Reactor {
            next_handler: 4,
            next_evented: 4,
            handlers: HashMap::new(),
            evthdlrs: HashMap::new(),
            eventeds: HashMap::new(),
        }
    }

    fn register_everything(&mut self, event_loop: &mut mio::EventLoop<Self>) {
        for (nm, ev) in self.eventeds.iter_mut() {
            ev.register(event_loop, mio::Token(*nm));
        }
    }

    fn reregister_everything(&mut self, event_loop: &mut mio::EventLoop<Self>) {
        for (nm, ev) in self.eventeds.iter_mut() {
            ev.reregister(event_loop, mio::Token(*nm));
        }
    }

    fn add_handler_raw(
        &mut self,
        nm: HandlerName,
        s: &'h mut Handler
    ) -> HandlerName {
        self.handlers.insert(nm, s);
        nm
    }

    pub fn add_handler(&mut self, s: &'h mut Handler) -> HandlerName {
        let name = self.next_handler;
        self.next_handler += 1;
        self.add_handler_raw(name, s)
    }

    fn add_evented_raw(
        &mut self,
        nm: EventedName,
        hn: HandlerName,
        s: Box<Evented>
    ) -> EventedName {
        self.evthdlrs.insert(nm, hn);
        self.eventeds.insert(nm, s);
        nm
    }

    pub fn add_evented(&mut self, hn: HandlerName, s: Box<Evented>) -> EventedName {
        let name = self.next_evented;
        self.next_evented += 1;
        self.add_evented_raw(name, hn, s)
    }

    pub fn run(&mut self) {
        let mut event_loop = mio::EventLoop::new().unwrap();
        self.register_everything(&mut event_loop);
        event_loop.run(self).unwrap();
    }

    pub fn connect<A: ToSocketAddrs>(
        &mut self,
        addr: A,
        hn: HandlerName
    ) -> io::Result<EventedName> {
        if !self.handlers.contains_key(&hn) {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "tried to use an invalid handler!"
            ));
        }

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
        let evented = EventedSocket::new(sock);

        Ok(self.add_evented(hn, Box::new(evented)))
    }

    pub fn queue_send(&mut self, nm: EventedName, data: &[u8]) {
        match self.eventeds.get_mut(&nm) {
            Some(e) => { e.queue_send(data); },
            None => error!("queued send for bad evented"),
        }
    }
}

impl<'h> mio::Handler for Reactor<'h> {
    type Timeout = ();
    type Message = ();

    fn ready(
        &mut self,
        event_loop: &mut mio::EventLoop<Self>,
        token: mio::Token,
        events: mio::EventSet
    ) {
        let (new_eventeds, queued_sends) = {
            let hn = match self.evthdlrs.get(&token.as_usize()) {
                Some(x) => x,
                None => {
                    error!("token for evented we don't know about");
                    return;
                }
            };

            let ev = match self.eventeds.get_mut(&token.as_usize()) {
                Some(x) => x,
                None => {
                    error!("token for evented we dn't know about");
                    return;
                }
            };

            let h = match self.handlers.get_mut(hn) {
                Some(x) => x,
                None => {
                    error!("evented registered with invalid handler");
                    return;
                }
            };

            let mut ctl = ReactorCtl {
                next_evented: &mut self.next_evented,
                new_eventeds: Vec::new(),
                queued_sends: Vec::new(),
            };

            if events.is_readable() {
                ev.recv_ready(token.as_usize(), &mut **h, &mut ctl);
            }

            if events.is_writable() {
                ev.send_ready(token.as_usize(), &mut **h, &mut ctl);
            }

            (ctl.new_eventeds, ctl.queued_sends)
        };

        for (nm, hn, e) in new_eventeds.into_iter() {
            self.add_evented_raw(nm, hn, e);
        }
        for (nm, data) in queued_sends.into_iter() {
            match self.eventeds.get_mut(&nm) {
                Some(e) => { e.queue_send(&data[..]); },
                None => error!("queued send for bad evented"),
            }
        }

        self.reregister_everything(event_loop);
    }
}

impl<'a> ReactorCtl<'a> {
    pub fn add_evented(&mut self, hn: HandlerName, s: Box<Evented>) -> EventedName {
        let name = *self.next_evented;
        *self.next_evented += 1;
        self.new_eventeds.push((name, hn, s));
        name
    }

    pub fn stop_everything(&mut self) {
        unimplemented!();
    }

    pub fn queue_send(&mut self, eid: EventedName, data: &[u8]) {
        self.queued_sends.push((eid, data.to_owned()));
    }
}

impl EventedSocket {
    pub fn new(sock: mio::tcp::TcpStream) -> EventedSocket {
        EventedSocket {
            sock: sock,
            outbuf: Vec::new()
        }
    }
}

impl Evented for EventedSocket {
    fn register<'h>(
        &self,
        event_loop: &mut mio::EventLoop<Reactor<'h>>,
        tok: mio::Token
    ) {
        let event_set = {
            let mut evs = mio::EventSet::readable();
            if self.outbuf.len() > 0 {
                evs = evs | mio::EventSet::writable();
            }
            evs
        };

        event_loop.register_opt(
            &self.sock,
            tok,
            event_set,
            mio::PollOpt::level()
        ).unwrap();
    }

    fn reregister<'h>(
        &self,
        event_loop: &mut mio::EventLoop<Reactor<'h>>,
        tok: mio::Token
    ) {
        event_loop.deregister(&self.sock).unwrap();
        self.register(event_loop, tok);
    }

    fn queue_send(&mut self, data: &[u8]) {
        self.outbuf.extend(data.iter().cloned());
    }

    fn recv_ready<'a>(
        &mut self,
        nm: EventedName,
        h: &mut Handler,
        rc: &mut ReactorCtl<'a>
    ) {
        let mut buf: [u8; RECV_BUF_SIZE] = unsafe { mem::uninitialized() };
        let size = self.sock.read(&mut buf[..]).unwrap();

        let mut ctl = EventedSocketCtl {
            name: nm,
            outbuf: &mut self.outbuf,
            rc: rc,
        };

        h.on_incoming_data(&mut ctl, &buf[..size]);
    }

    fn send_ready<'a>(
        &mut self,
        _nm: EventedName,
        _h: &mut Handler,
        _rc: &mut ReactorCtl<'a>
    ) {
        let size = self.sock.write(&self.outbuf[..]).unwrap();

        let outbuf = self.outbuf.split_off(size);
        self.outbuf = outbuf;
    }
}

struct EventedSocketCtl<'a, 'b: 'a> {
    name: EventedName,
    outbuf: &'a mut Vec<u8>,
    rc: &'a mut ReactorCtl<'b>,
}

impl<'a, 'b: 'a> EventedCtl<'b> for EventedSocketCtl<'a, 'b> {
    fn name(&self) -> EventedName { self.name }

    fn reactor(&mut self) -> &mut ReactorCtl<'b> { self.rc }

    fn queue_send(&mut self, data: &[u8]) {
        self.outbuf.extend(data.iter().cloned());
    }

    fn stop_recv(&mut self) {
        unimplemented!();
    }

    fn stop_send(&mut self) {
        unimplemented!();
    }
}
