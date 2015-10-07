# *miau*

![build status](http://bob.ajitek.net/r/miau/badge.png)
![build status](http://bob.ajitek.net/r/miau/badge.png)
![build status](http://bob.ajitek.net/r/miau/badge.png)
[repo at bob the builder](http://bob.ajitek.net/r/miau)
![build status](http://bob.ajitek.net/r/miau/badge.png)
![build status](http://bob.ajitek.net/r/miau/badge.png)
![build status](http://bob.ajitek.net/r/miau/badge.png)

`miau` is a community IRC bot project with a very hacked-together continuous
integration pipeline allowing code changes to the `master` branch of the repo
to be deployed to the running instance within minutes.

Contact the maintainers at `irc.canternet.org #miau-dev`

# v-- just for developers --v

To keep the project somewhat coherent, a focus on documentation availability is
key. For now high-level documentation is going in the `README.md`, until we
decide that a wiki of some kind (probably just the GitHub wiki) is sufficient.

## The build system

`miau` is a Cargo-based project, providing easy and consistent access to all
crates on [crates.io](http://crates.io) or public git repositories. The build
is run in release mode using the nightly Rust toolchain, managed by the
excellent [multirust](https://github.com/brson/multirust) tool. It's
recommended that developers use a similar setup for compiling the project,
as it's both easier to predict what a CI build will look like, as well as
easier to develop in general.

### Nightly builds

Because the Rust ecosystem moves so quickly, and nightly releases are generally
unstable, `bob` is set up to perform nightly builds of the repository as well.
If no build has successfully completed in the last 24 hours, `bob` starts a
build with the most recent successfully built revision. This is intended to
detect changes to the build environment that would break previously correct
code.

Note that, with the pipeline configuration described below, complete promotion
to the production environment can still fail after a successful build. However,
it's the opinion of this developer that, with the safety guarantees of Rust and
general aversion of library authors to strange behavioral changes, issues with
the build should be detected in the majority of cases before leaving the build
bot. Of course, there is no replacement for good development discipline, and
bad deployments need to be expected as well.

## Configuration

To support development, staging, and production instances, a simple tiered
configuration system is used. The base configuration lives in
`config/miau-prod.toml`. As the name implies, this is also the production
configuration. Changes to this file should be treated with care. To support
alternate setups, an override file can be specified whose settings supersede
the base configuration. When looking up a setting, the configuration system
first checks the override, falling back to the base configuration if no
setting was found.

This gets a bit tricky when dealing with collections of settings, such as a
list and tables. In the situtation where a collection is present in both the
base *and* override configuration, the override collection will be preferred.
As a result, contributors should be mindful of the ways they fetch data from
the configuration system and think about the implications of an override.

## The CI pipeline

Currently, the pipeline is structured like this:

* The build bot at [bob.ajitek.net](http://bob.ajitek.net/r/miau) picks up
  changes to the `master` branch of the GitHub repo and kicks off a build.

* When the build succeeds, the new repo is pushed to a versioned Amazon S3
  bucket `bob-upload-prod` where Amazon CodePipeline detects a change and
  starts a deployment with Amazon CodeDeploy.

* That's it!

So right now, `appspec.yml` describes the **production deployment
configuration**, which is fine at the moment but a little boring.

### The future

*This part does not actually apply right now!*

Eventually some sort of integration test suite will be developed that will
verify the bot runs correctly in the staging deployment and, if it does,
promote it to production for deployment. When that's done, checking in working
code will trigger the following sequence of events:

* GitHub sends a push notification to `bob`, who pulls the change and runs a
  build. The built assets are uploaded to S3.

* CodePipeline detects the new assets and creates a CodeDeploy deployment to the
  staging host.

* The deployment scripts detect we're in the staging environment and run a
  series of tests.

* The tests pass and the deployment completes successfully. CodePipeline then
  creates a CodeDeploy deployment to the production host.

* The deployment scripts detect we're in the production environment and flip
  to the new instance.
