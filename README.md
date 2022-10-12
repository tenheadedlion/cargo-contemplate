# A Cargo Subcommand For Fast Project Setup

## Install

```shell
cargo install cargo-contemplate
```

## Usage

```shell
cargo-contemplate <class>
```

Currently supported classes: 

- phat-contract
- phat-contract-with-sideprog

## Examples


```shell
cargo contemplate phat-contract a-starter-project
```

The command above will create a directory with the name "phat-contract-start" in the current directory

## Todos

- [ ] clap seems to mess up `cargo-x` and `cargo x`
- [ ] move interactive
- [ ] support sparse-checkout
