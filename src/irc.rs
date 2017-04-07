//! IRC handling functions

use std::fmt;
use std::str::CharIndices;
use std::iter::Peekable;

/// Helper for the message parser
struct Scanner<'a> {
    s: &'a str,
    c: Peekable<CharIndices<'a>>,
}

/// The parsed form of an IRC message.
#[derive(PartialEq)]
pub struct Message<'a> {
    pub src: MessageSource<'a>,
    pub verb: &'a str,
    pub args: Vec<&'a str>,
}

/// The parsed source of an IRC message.
#[derive(PartialEq)]
pub enum MessageSource<'a> {
    Missing,
    User(&'a str, Option<&'a str>, Option<&'a str>),
    Server(&'a str),
}

impl<'a> Scanner<'a> {
    fn new(s: &str) -> Scanner {
        Scanner { s: s, c: s.char_indices().peekable() }
    }

    fn byte_pos(&mut self) -> usize {
        self.c.peek().map(|x| x.0).unwrap_or(self.s.len())
    }

    fn peek(&mut self) -> char {
        self.c.peek().map(|x| x.1).unwrap_or('\n')
    }

    fn empty(&mut self) -> bool {
        self.byte_pos() >= self.s.len()
    }

    fn skip(&mut self) {
        let _ = self.c.next();
    }

    fn skip_while<F>(&mut self, f: F) where F: Fn(char) -> bool {
        while !self.empty() && f(self.peek()) { self.skip(); }
    }

    fn skip_spaces(&mut self) {
        self.skip_while(|c| c.is_whitespace());
    }

    fn chomp(&mut self) -> &'a str {
        self.skip_spaces();
        let start = self.byte_pos();
        self.skip_while(|c| !c.is_whitespace());
        let end = self.byte_pos();
        self.skip_spaces();

        &self.s[start..end]
    }

    fn chomp_remaining(&mut self) -> &'a str {
        let start = self.byte_pos();
        while !self.empty() { self.skip(); }
        &self.s[start..]
    }
}

impl<'a> Message<'a> {
    /// Parses the byte slice into a `Message`
    pub fn parse(spec: &'a str) -> Result<Message<'a>, &'static str> {
        let mut scan = Scanner::new(spec);

        scan.skip_spaces();

        let src = if scan.peek() == ':' {
            scan.skip();
            MessageSource::parse(scan.chomp())
        } else {
            MessageSource::Missing
        };

        let verb = scan.chomp();

        let mut args = Vec::new();
        while !scan.empty() {
            args.push(if scan.peek() == ':' {
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
        try!(write!(f, "Message({:?}, {:?}", self.src, self.verb));
        for s in self.args.iter() {
            try!(write!(f, ", {:?}", s));
        }
        try!(write!(f, ")"));
        Ok(())
    }
}

impl<'a> MessageSource<'a> {
    pub fn parse(spec: &'a str) -> MessageSource<'a> {
        use self::MessageSource::*;

        // If there's a bug in this code, you're probably better off rewriting
        // the whole thing.

        // we shouldn't ever be parsing an empty source string, but just in
        // case, handle it as Missing.
        if spec.len() == 0 { return Missing }

        let delimit = |c: &char| { *c == '.' || *c == '!' || *c == '@' };

        // match on the first delimiter
        match spec.chars().filter(delimit).nth(0) {
            Some('!') | Some('@') => {
                let ex = spec.chars().position(|c| c == '!');
                let at = spec.chars().position(|c| c == '@');

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
            Some('.') => Server(spec),
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
            User(ref n, ref u, ref h) =>
                write!(f, "User({}!{}@{})", n, u.unwrap_or("?"), h.unwrap_or("?")),
            Server(ref s) => write!(f, "Server({})", s),
        }
    }
}

#[test]
fn message_source_parse_server() {
    use self::MessageSource::*;

    assert_eq!(MessageSource::parse("my.host"),
        Server("my.host"));
}

#[test]
fn message_source_parse_user() {
    use self::MessageSource::*;

    assert_eq!(MessageSource::parse("miau"),
        User("miau", None, None));
    assert_eq!(MessageSource::parse("miau!~u"),
        User("miau", Some("~u"), None));
    assert_eq!(MessageSource::parse("miau@h.ost"),
        User("miau", None, Some("h.ost")));
    assert_eq!(MessageSource::parse("miau!~u@h.ost"),
        User("miau", Some("~u"), Some("h.ost")));

    // servers should never do this, but test it anyway just in case.
    assert_eq!(MessageSource::parse("miau@h.ost!~u"),
        User("miau", Some("~u"), Some("h.ost")));
}

#[test]
fn message_parse_no_source() {
    assert_eq!(Message {
        src: MessageSource::Missing,
        verb: "PING",
        args: vec!["123"],
    }, Message::parse("PING 123").unwrap());
}

#[test]
fn message_parse_trailing() {
    assert_eq!(Message {
        src: MessageSource::Missing,
        verb: "PING",
        args: vec!["this has spaces"],
    }, Message::parse("PING :this has spaces").unwrap());
}

#[test]
fn message_parse_with_spaces() {
    assert_eq!(Message {
        src: MessageSource::Missing,
        verb: "PING",
        args: vec!["this", "has", "spaces"],
    }, Message::parse("PING this has spaces").unwrap());
}

#[test]
fn message_parse_with_many_extra_spaces() {
    // not technically to-spec
    assert_eq!(Message {
        src: MessageSource::Server("h.ost"),
        verb: "PING",
        args: vec!["this", "has", "very many spaces  "],
    }, Message::parse("  :h.ost  PING     this   has   :very many spaces  ").unwrap());
}

#[test]
fn message_parse_with_many_extra_spaces_and_no_trailing() {
    // not technically to-spec
    assert_eq!(Message {
        src: MessageSource::Server("h.ost"),
        verb: "PING",
        args: vec!["this", "has", "very", "many", "spaces"],
    }, Message::parse("  :h.ost  PING     this   has   very many spaces  ").unwrap());
}

#[test]
fn message_parse_with_source() {
    assert_eq!(Message {
        src: MessageSource::Server("h.ost"),
        verb: "PING",
        args: vec!["this", "has spaces"],
    }, Message::parse(":h.ost PING this :has spaces").unwrap());
}
