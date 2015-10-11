//! IRC handling functions

use std::fmt;

/// Helper for the message parser
struct Scanner<'a> {
    s: &'a [u8],
    i: usize,
}

/// The parsed form of an IRC message.
#[derive(PartialEq)]
pub struct Message<'a> {
    pub src:   MessageSource<'a>,
    pub verb:  &'a [u8],
    pub args:  Vec<&'a [u8]>,
}

/// The parsed source of an IRC message.
#[derive(PartialEq)]
pub enum MessageSource<'a> {
    Missing,
    User(&'a [u8], Option<&'a [u8]>, Option<&'a [u8]>),
    Server(&'a [u8]),
}

impl<'a> Scanner<'a> {
    fn new(s: &[u8]) -> Scanner {
        Scanner {
            s: s,
            i: 0,
        }
    }

    fn peek(&self) -> u8 {
        if self.i < self.s.len() {
            self.s[self.i]
        } else {
            0
        }
    }

    fn empty(&self) -> bool {
        self.i >= self.s.len()
    }

    fn skip(&mut self) {
        self.i += 1;
    }

    fn skip_spaces(&mut self) {
        while !self.empty() && (self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }

    fn chomp(&mut self) -> &'a [u8] {
        self.skip_spaces();
        let start = self.i;
        while !self.empty() && !(self.s[self.i] as char).is_whitespace() {
            self.i += 1;
        }
        let end = self.i;
        self.skip_spaces();

        &self.s[start..end]
    }

    fn chomp_remaining(&mut self) -> &'a [u8] {
        let i = self.i;
        self.i = self.s.len();
        &self.s[i..]
    }
}

impl<'a> Message<'a> {
    /// Parses the byte slice into a `Message`
    pub fn parse(spec: &'a [u8]) -> Result<Message<'a>, &'static str> {
        let mut scan = Scanner::new(spec);

        scan.skip_spaces();

        let src = if scan.peek() == b':' {
            scan.skip();
            MessageSource::parse(scan.chomp())
        } else {
            MessageSource::Missing
        };

        let verb = scan.chomp();

        let mut args = Vec::new();
        while !scan.empty() {
            args.push(if scan.peek() == b':' {
                scan.skip();
                scan.chomp_remaining()
            } else {
                scan.chomp()
            });
        }

        Ok(Message {
            src:  src,
            verb: verb,
            args: args
        })
    }
}

impl<'a> fmt::Debug for Message<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "Message({:?}, {:?}", self.src,
            String::from_utf8_lossy(self.verb)));
        for s in self.args.iter() {
            try!(write!(f, ", {:?}", String::from_utf8_lossy(s)));
        }
        try!(write!(f, ")"));
        Ok(())
    }
}

impl<'a> MessageSource<'a> {
    pub fn parse(spec: &'a [u8]) -> MessageSource<'a> {
        use self::MessageSource::*;

        // If there's a bug in this code, you're probably better off rewriting
        // the whole thing.

        // we shouldn't ever be parsing an empty source string, but just in
        // case, handle it as Missing.
        if spec.len() == 0 { return Missing }

        let delimit = |b: &&u8| { **b == b'.' || **b == b'!' || **b == b'@' };

        // match on the first delimiter
        match spec.iter().filter(delimit).nth(0) {
            Some(&b'!') | Some(&b'@') => {
                let ex = spec.iter().position(|b| *b == b'!');
                let at = spec.iter().position(|b| *b == b'@');

                // this is horrible and I'm sorry but hopefully I get it right
                // the first time and nobody has to look at it ever again.
                match ex {
                    Some(exi) => match at {
                        Some(ati) => {
                            if exi < ati {
                                User(
                                    &spec[..exi],
                                    Some(&spec[exi+1..ati]),
                                    Some(&spec[ati+1..])
                                )
                            } else {
                                User(
                                    &spec[..ati],
                                    Some(&spec[exi+1..]),
                                    Some(&spec[ati+1..exi])
                                )
                            }
                        },
                        None => User(&spec[..exi], Some(&spec[exi+1..]), None)
                    },
                    None => match at {
                        Some(ati) => User(&spec[..ati], None, Some(&spec[ati+1..])),
                        None      => User(&spec[..],    None, None),
                    },
                }
            },
            Some(&b'.') => Server(spec),
            Some(_) => { error!("couldn't parse source!"); Missing }
            None => User(spec, None, None)
        }
    }
}

impl<'a> fmt::Debug for MessageSource<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::MessageSource::*;

        match *self {
            Missing => write!(f, "Missing"),
            User(ref n, ref u, ref h) => write!(f, "User({}!{}@{})",
                String::from_utf8_lossy(n),
                String::from_utf8_lossy(if let &Some(us) = u { us } else { b"" }),
                String::from_utf8_lossy(if let &Some(hs) = h { hs } else { b"" }),
            ),
            Server(ref s) => write!(f, "Server({})",
                String::from_utf8_lossy(s)
            ),
        }
    }
}

#[test]
fn message_source_parse_server() {
    use self::MessageSource::*;

    assert_eq!(MessageSource::parse(b"my.host"),
        Server(b"my.host"));
}

#[test]
fn message_source_parse_user() {
    use self::MessageSource::*;

    assert_eq!(MessageSource::parse(b"miau"),
        User(b"miau", None, None));
    assert_eq!(MessageSource::parse(b"miau!~u"),
        User(b"miau", Some(b"~u"), None));
    assert_eq!(MessageSource::parse(b"miau@h.ost"),
        User(b"miau", None, Some(b"h.ost")));
    assert_eq!(MessageSource::parse(b"miau!~u@h.ost"),
        User(b"miau", Some(b"~u"), Some(b"h.ost")));

    // servers should never do this, but test it anyway just in case.
    assert_eq!(MessageSource::parse(b"miau@h.ost!~u"),
        User(b"miau", Some(b"~u"), Some(b"h.ost")));
}

#[test]
fn message_parse_no_source() {
    assert_eq!(Message {
        src: MessageSource::Missing,
        verb: b"PING",
        args: vec![b"123"],
    }, Message::parse(b"PING 123").unwrap());
}

#[test]
fn message_parse_trailing() {
    assert_eq!(Message {
        src: MessageSource::Missing,
        verb: b"PING",
        args: vec![b"this has spaces"],
    }, Message::parse(b"PING :this has spaces").unwrap());
}

#[test]
fn message_parse_with_spaces() {
    assert_eq!(Message {
        src: MessageSource::Missing,
        verb: b"PING",
        args: vec![b"this", b"has", b"spaces"],
    }, Message::parse(b"PING this has spaces").unwrap());
}

#[test]
fn message_parse_with_source() {
    assert_eq!(Message {
        src: MessageSource::Server(b"h.ost"),
        verb: b"PING",
        args: vec![b"this", b"has spaces"],
    }, Message::parse(b":h.ost PING this :has spaces").unwrap());
}
