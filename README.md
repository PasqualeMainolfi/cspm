# cspm: A Modern Package Manager for Csound

**NOTE: THIS PROJECT IS CURRENTLY A WORK IN PROGRESS. A REMOTE REGISTRY IS NOT YET AVAILABLE, BUT IT WILL BE IMPLEMENTED SOON!**      

**cspm** is a modern, fast, and deterministic package manager for the [Csound](https://csound.com/), written in Rust.

It brings modern dependency management (similar to `cargo` or `npm`) to the Csound ecosystem.  
With `cspm`, you can easily initialize projects, manage third-party modules (UDOs), ensure reproducible builds via lockfiles, and publish your own DSP tools to the community.

---

## Features

- **Simple Dependency Management**: Add, update, or remove Csound modules with a single command.
- **Deterministic Builds**: Uses a Cspm.lock file with cryptographic checksums (SHA-256) to ensure your project produces exactly the same results on any machine.
- **Recursive Dependency Resolution**: Automatically resolves and installs complete dependency trees.
- **Global Module Cache**: Modules are downloaded only once and stored in a global cache, allowing subsequent projects to reuse them instantly.
- **Local or Global Installation**: Modules can be installed either locally within a project or globally for system-wide use.
- **Module Discovery**: Search the registry to quickly find available modules and versions.
- **TOML-based Manifests**: Clean and human-readable Cspm.toml files define your project configuration and dependencies.
- **Built-in Runner**: Run your Csound projects directly with cspm run.

---

## Installation

*Note: Pre-compiled binaries will be available in the Releases tab soon.*

If you have [Rust and Cargo](https://rustup.rs/) installed, you can build and install `cspm` from source:

```bash
git clone <https://github.com/PasqualeMainolfi/cspm.git>
cd cspm
cargo install --path .
```

Verify the installation

```bash
cspm --version
```

Add a community module to your project:

```bash
cspm add module
```

```bash
cspm add module1, module2, ...
```

## Cspm.toml file

```toml
[package]
name = "my_synth"
version = "0.1.0"
description = "An awesome FM synthesizer"
authors =["<you@example.com>"]
license = "MIT"
include = ["src"]

[dependencies]
module1 = "1.0.0"
module2 = "2.1.3"

[main]
csd = "src/main.csd"
# orc = "src/main.orc"
# sco = "src/main.sco"
```

## Command reference

```text
﬌  ../cspm/target/release/cspm --help
Hello, from cspm!
A modern package manager for Csound

Usage: cspm [OPTIONS] <COMMAND>

Commands:
  init, -i, --init        Create a new Csound project
  add, -a, --add          Add dependencies to the project
  reinstall               Reinstall dependencies to the project
  remove, -r, --remove    Remove dependencies from the project
  update, -u, --update    Update the project's dependencies
  cache, -c, --cache      Manage cspm cache
  sync                    Check the project's environment status
  build, -b, --build      Build project from manifest or lock file
  publish, --publish      Publish Csound module
  run, --run              Run Csound project
  search                  Display module info
  version, -v, --version  Display cspm version
  help                    Print this message or the help of the given subcommand(s)

Options:
  -g, --global  Use global environment
  -f, --force   Force removal of dependencies
  -h, --help    Print help
```
