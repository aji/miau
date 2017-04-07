# *miau*

  * [at Travis](https://travis-ci.org/aji/miau)
    * ![travis badge](https://travis-ci.org/aji/miau.svg?branch=master) `master`
  * [documentation](http://aji.github.io/miau)

`miau` is a community IRC bot project with a very hacked-together continuous
integration pipeline allowing code changes to the `master` branch of the repo
to be deployed to the running instance within minutes.

Contact the maintainers at `irc.canternet.org #miau-dev`

# v-- just for developers --v

To keep the project somewhat coherent, a focus on documentation availability is
key. For now high-level documentation is going in the `README.md`, until we
decide that a wiki of some kind (probably just the GitHub wiki) is sufficient.

## Development workflow

First, you should copy `config/miau-dev.toml.demo` to `config/miau-dev.toml`.
By default, the bot looks for an override file at `config/miau-dev.toml`. This
is to prevent people from accidentally using the production configuration when
running the bot. Since `config/miau-dev.toml` is in `.gitignore`, you can make
changes to it without worrying about accidentally committing them.

To compile and run the bot with the development configuration, run something
like the following:

    $ cargo build
    $ target/debug/miau

`miau` looks for the name of the configuration overlay in the `MIAU_OVERLAY`
environment variable. For example, to run the bot with the demo development
overlay,

    $ MIAU_OVERLAY=config/miau-dev.toml.demo target/debug/miau

or

    $ export MIAU_OVERLAY=config/miau-dev.toml.demo
    $ target/debug/miau

If you specifically want to use the production configuration, you can specify
`config/miau-prod.toml` or `/dev/null` as the value of `MIAU_OVERLAY`. If you
want to test a different base configuration file, `miau` looks in `MIAU_CONFIG`
for the path. This defaults to `config/miau-prod.toml`, but no warning is
printed when using the default.

It's recommended to install `rustup`, `travis-cargo`, and the Heroku
Toolbelt for the most accurate testing setup. Thankfully, this is usually only
necessary when testing changes to the Travis and Heroku integration, but it can
be useful to verify that a set of changes will correctly build and run in
production. To build:

    $ rustup default nightly
    $ travis-cargo

To run,

    $ cargo build --release
    $ MIAU_OVERLAY=my-overlay.toml heroku local

## The build system

`miau` is a Cargo-based project, providing easy and consistent access to all
crates on [crates.io](http://crates.io) or public git repositories. Builds are
first handled by Travis, and passing builds are handed off to Heroku to be
built a second time and ultimately deployed. Because of all the places this can
fail, work is being done to have notifications delivered to IRC in a timely
manner.

## Configuration

To support development, staging, and production instances, a simple tiered
configuration system is used. The base configuration lives in
`config/miau-prod.toml`. As the name implies, this is also the production
configuration. Changes to this file should be treated with care. To support
alternate setups, an overlay file can be specified whose settings supersede
the base configuration. When looking up a setting, the configuration system
first checks the overlay, falling back to the base configuration if no
setting was found.

This gets a bit tricky when dealing with collections of settings, such as a
list and tables. In the situtation where a collection is present in both the
base *and* overlay configuration, the overlay collection will be preferred.
As a result, contributors should be mindful of the ways they fetch data from
the configuration system and think about the implications of an overlay.

## Code style

There aren't many hard and fast rules, just try to stick to what you see in the
surrounding code. The most important rule is that your code be readable, but
this criteria is differently for everybody, and there's a lot of variation in
how people will interpret it. Since this is a community project, there are some
guidelines to ensure that everybody is on the same page:

  * **DEFINITELY DO:**
      * **Make sure your code is readable.** What that means will be different
        to everybody, but code that is clearly hard to follow will be rejected.
        Don't be afraid to use extra lines or spacing. There's no points for
        low line count in projects with multiple contributors.
      * **Indent with 4 spaces, and spaces only.** It's the year 201x, if
        you're using an editor that makes it hard to do this, then get a
        better editor. I'm 100% serious. This is going to be pretty strongly
        enforced across all Rust projects everywhere so it's a good habit to
        get in anyway.
      * **Stick to Rust naming conventions.** That means CamelCase for type
        and trait names, UPPER_SNAKE_CASE for `const` things, and snake_case
        for just about everything else. The compiler will print warnings if
        you don't use the conventions. Exceptions to this are allowed if there's
        a clear gain in readability, but otherwise use names that match the
        standard.
      * **Avoid block comments.** That means using line comments, i.e. `//`
        for normal comments, `///` and `//!` for doc comments. There's not
        really a reason for this, but mixed commenting styles just look really
        bad.
      * **`use` whole modules if possible.** Rust lets you do `use
        something::far::away` and then refer to things in the module as
        `away::This` and `away::That`.  If you do want to `use` things from the
        module, then `use` them explicitly like `use something::{This, That}`
        instead of globbing them.

  * **DEFINITELY DO NOT:**
      * **Use tons of glob `use`.** Glob `use` statements (e.g.
        `use foo::bar::*`) clutter the namespace. This rule really only applies
        to file-level `use`, (so a `use` at the top of a function that imports
        all of an `enum`'s variants for use in that function is fine.) Again,
        what uses of file-level `use` globbing is acceptable is a preferential
        thing, so have good judgement.
      * **Use lines longer than 80 characters.** The wider Rust community has
        settled on 100 lines as a maximum, but 80 this is more of a personal
        preference. I can't comfortably view files with many lines longer than
        80 characters, as is the case for a few others, so if you need to go
        past 80, do so sparingly. If 80 is regularly not enough room, then
        chances are you have some refactoring to do anyway.
      * **Use crap names.** Again, this is going to be different to everybody,
        but certain choices are obviously bad. Just be smart about it.

Things not on these lists are generally up to your discretion. Again,
readability is the most important factor, and exceptions can be made to all
these rules if there is a *clear* benefit to readability. ("It reads better for
me this way" arguments don't count.)

## Code organization

Some simple guidelines for keeping code well-organized follow. Again, like
every rule about writing code, exceptions are made if code is clearly more
understandable that way.

  * Modules are cheap, don't be afraid to use them. Rust lets you re-export
    things, i.e. `pub use foo::bar::Thing` makes `Thing` a symbol that can be
    imported from within your module. This is a really great feature to use if
    you want to split a module's functionality across several submodules and
    make key entry points easily available to the module's consumers.

  * Remember to write documentation and tests! Don't worry about being too
    thorough until you're finishing up, but finished code (i.e. code to be
    pushed to `master`) should be well-documented and well-tested. The
    integrity of the CI pipeline relies on good tests, and documentation is a
    key way of communicating with other developers about your code. The builds
    include documentation, which developers are expected to keep relevant and
    helpful.

  * Avoid `pub` if it's not needed. Thankfully Rust makes it hard to
    accidentally make something visible to everybody, but putting `pub` on
    all members of a `struct`, for example, is just bad practice. If you want
    users to directly access a `struct`'s members, it's better in the long
    run to provide immutable and mutable accessors, e.g.
    `pub fn a_field(&self) -> &Type { &self.a_field }`

  * Avoid things that may panic, like `.unwrap()`. If you're not absolutely
    certain that `.unwrap()` will succeed in all but the most exceptional
    cases, don't use it. At the very least, add a `TODO` comment.

  * Prefer `Result` as a return type for expressing possible failure instead
    of `Option`. `Result` can be used with the `try!` macro, and from a semantic
    standpoint represents the result of an operation that can fail, versus
    `Option` which represents the possible absence of a value. If you define
    a custom error type for use with `Result`, it can be helpful to define
    a type alias for `Result` that captures that, i.e.
    `pub type MyResult<T> = Result<T, MyError>;`
