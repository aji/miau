use irc::Message;

use network::Network;
use network::Output;

/// Handles a command in the given command handling context.
pub fn handle_command<X: Context>(ctx: &mut X, cmd: &str, _args: &str) {
    match cmd {
        "version" => {
            ctx.reply(format!("i am {} v{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ));
        }

        _ => {
            ctx.reply_warn(format!("unknown command: {}", cmd));
        }
    }
}

fn chomp_index(s: &str) -> Option<(usize, usize)> {
    let mut end = None;

    for (i, c) in s.char_indices() {
        if end.is_none() && c.is_whitespace() {
            end = Some(i);
        } else if end.is_some() && !c.is_whitespace() {
            return end.map(|j| (j, i));
        }
    }

    end.map(|j| (j, s.len()))
}

/// Helper method for handling messages that come from an IRC network. This method may or may
/// not actually call `handle_command`, since the message may not be formatted with the
/// correct command syntax.
pub fn handle_irc<'m, T: Output>(net: &mut Network, out: &mut T, m: Message<'m>) {
    // quick sanity check, this should be a PRIVMSG
    if m.verb != "PRIVMSG" {
        warn!("handle_irc called with something other than PRIVMSG: {:?}", m);
        return;
    }

    let my_nick = match net.current_nick() {
        Some(s) => s,
        None => {
            warn!("handle_irc called while we have no nickname");
            return;
        }
    };

    let text = m.args[1];
    let mut start_at = None;

    // if this is a private message, then the entire line is (probably) the command
    if !m.args[0].starts_with("#") {
        start_at = Some(0);
    }

    // line might start with our name.
    if text.starts_with(my_nick) {
        start_at = chomp_index(text).map(|x| x.1);
    }

    // line might start with ! which we can easily skip
    if text.starts_with("!") {
        start_at = Some(1);
    }

    let spec = match start_at {
        Some(i) => &text[i..],
        None => return
    };

    let (cmd_ends_at, args_start_at) = chomp_index(spec).unwrap_or((spec.len(), spec.len()));
    let cmd = &spec[..cmd_ends_at];
    let args = &spec[args_start_at..];

    let mut ctx = IrcContext::new(out, m);
    handle_command(&mut ctx, cmd, args);
}

/// A trait defining the context in which commands are handled. Commands must interact with the
/// world through an implementation of Context.
pub trait Context {
    /// A normal response to a command. These are generally guaranteed to be seen by whoever or
    /// whatever issued the command.
    fn reply<S: AsRef<str>>(&mut self, line: S);

    /// An error message. These might be treated or formatted differently, depending on context.
    /// By default, errors are treated identically to normal replies.
    fn reply_error<S: AsRef<str>>(&mut self, line: S) {
        self.reply(line);
    }

    /// A warning message. These are ignored by default, but might be treated similarly to an
    /// error, depending on context.
    fn reply_warn<S: AsRef<str>>(&mut self, _line: S) {
        // warnings aren't printed by default
    }
}

struct IrcContext<'m, T: 'm> {
    out: &'m mut T,
    reply_to: &'m str,
    reply_prefix: Option<&'m str>
}

impl<'m, T: Output> IrcContext<'m, T> {
    fn new(out: &'m mut T, m: Message<'m>) -> IrcContext<'m, T> {
        if m.args[0].starts_with("#") {
            let reply_prefix = Some(m.src.short_name());
            IrcContext { out: out, reply_to: m.args[0], reply_prefix: reply_prefix }
        } else {
            IrcContext { out: out, reply_to: m.src.short_name(), reply_prefix: None }
        }
    }
}

impl<'m, T: Output> Context for IrcContext<'m, T> {
    fn reply<S: AsRef<str>>(&mut self, line: S) {
        match self.reply_prefix {
            Some(prefix) => {
                let full_line = format!("{}: {}", prefix, line.as_ref());
                self.out.PRIVMSG(self.reply_to, full_line);
            },
            None => {
                self.out.NOTICE(self.reply_to, line.as_ref());
            }
        }
    }

    fn reply_warn<S: AsRef<str>>(&mut self, line: S) {
        // print warning messages for contexts without a prefix. right now this
        // is just private messages.
        if self.reply_prefix.is_none() {
            self.reply(line);
        }
    }
}
