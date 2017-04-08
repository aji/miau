//! IRC network state management and protocol handling.
//!
//! Most socket-level nastiness is in bot.rs

use commands;
use environment::Env;
use irc::Message;

pub struct Network {
    env: Env,
    state: State,
}

#[derive(Clone)]
enum State {
    Registering(Registration),
    Active(Active),
}

#[derive(Clone)]
struct Registration {
    last_requested_nick: String,
}

#[derive(Clone)]
struct Active {
    nick: String,
}

impl Network {
    pub fn register<T: Output>(env: Env, out: &mut T) -> Network {
        let nick = env.conf_str("irc.nick").unwrap_or("miau").to_string();

        out.NICK(&nick);
        out.USER("miau", env!("CARGO_PKG_HOMEPAGE"));

        let reg = Registration { last_requested_nick: nick, };
        Network { env: env, state: State::Registering(reg) }
    }

    pub fn current_nick(&self) -> Option<&str> {
        match &self.state {
            &State::Active(ref act) => Some(&act.nick[..]),
            _ => None,
        }
    }

    pub fn handle_message<'m, T: Output>(&mut self, out: &mut T, m: Message<'m>) {
        if m.verb == "PING" {
            out.send(format!("PONG :{}", m.args[0]));
            return;
        }

        // TODO: this is gross. please fix. remember that methods like current_nick()
        // query the current state. maybe Cow?
        let prev_state = self.state.clone();
        let next_state = prev_state.handle(self, out, m);
        self.state = next_state;
    }

    fn for_each_autojoin_chan<F: FnMut(&str)>(&self, mut f: F) {
        if let Some(chans) = self.env.conf_array("irc.channels") {
            for c in chans.iter().filter_map(|c| c.as_str()) {
                f(c);
            }
        } else {
            f("#miau-dev");
        }
    }

    fn on_become_active<T: Output>(&mut self, out: &mut T) {
        self.for_each_autojoin_chan(|c| out.JOIN(c));
    }
}

#[allow(non_snake_case)]
pub trait Output {
    fn send(&mut self, line: String);

    fn NICK<S: AsRef<str>>(&mut self, nick: S) {
        self.send(format!("NICK {}", nick.as_ref()));
    }

    fn USER<S: AsRef<str>, T: AsRef<str>>(&mut self, ident: S, gecos: T) {
        self.send(format!("USER {} * * :{}", ident.as_ref(), gecos.as_ref()));
    }

    fn JOIN<S: AsRef<str>>(&mut self, chan: S) {
        self.send(format!("JOIN {}", chan.as_ref()));
    }

    fn NOTICE<S: AsRef<str>, T: AsRef<str>>(&mut self, target: S, text: T) {
        self.send(format!("NOTICE {} :{}", target.as_ref(), text.as_ref()));
    }

    fn PRIVMSG<S: AsRef<str>, T: AsRef<str>>(&mut self, target: S, text: T) {
        self.send(format!("PRIVMSG {} :{}", target.as_ref(), text.as_ref()));
    }
}

impl State {
    fn handle<'m, T: Output>(self, net: &mut Network, out: &mut T, m: Message<'m>) -> State {
        match self {
            State::Registering(reg) => reg.handle(net, out, m),
            State::Active(act) => act.handle(net, out, m),
        }
    }
}

impl Registration {
    fn handle<'m, T: Output>(mut self, net: &mut Network, out: &mut T, m: Message<'m>) -> State {
        match m.verb {
            "001" => { // RPL_WELCOME
                net.on_become_active(out);
                let my_nick = m.args[0].to_string();
                debug!("my nick is {}", my_nick);
                return State::Active(Active { nick: my_nick });
            },

            "433" => { // nickname in use
                let next_nick = format!("{}_", self.last_requested_nick);
                out.NICK(&next_nick);
                self.last_requested_nick = next_nick;
            },

            _ => { }
        }

        State::Registering(self)
    }
}

impl Active {
    fn handle<'m, T: Output>(self, net: &mut Network, out: &mut T, m: Message<'m>) -> State {
        match m.verb {
            "PRIVMSG" => commands::handle_irc(net, out, m),
            _ => { }
        }

        State::Active(self)
    }
}
