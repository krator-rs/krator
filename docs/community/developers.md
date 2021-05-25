# Developer guide

This guide explains how to set up your environment for developing Krator.

## Prerequisites

To build Krator, you will need

- The latest stable version of Rust
- openssl (Or use the [`rustls-tls`](#building-without-openssl) feature)
- git

If you want to test Krator, you will also require

- A Kubernetes cluster.
- [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl/)

## Building

We use `cargo` to build our programs:

```console
$ cargo build
```

### Building without openssl

If you are on a system that doesn't have OpenSSL (or has the incorrect version),
you have the option to build Krator using the Rustls project (Rust native TLS
implementation):

```console
$ cargo build --no-default-features --features rustls-tls
```

The same flags can be passed to `cargo run` if you want to just [run](#running)
the project instead.

#### Caveats

The underlying dependencies for Rustls do not support certs with IP SANs
(subject alternate names). Because of this, the serving certs requested during
bootstrap will not work for local development options like minikube or KinD as
they do not have an FQDN

### Building on WSL (Windows Subsystem for Linux)

You can build Krator on WSL but will need a few prerequisites that aren't
included in the Ubuntu distro in the Microsoft Store:

```console
$ sudo apt install build-essential libssl-dev pkg-config
```

**NOTE:** We've had mixed success developing Krutorstlet on WSL. It has been
successfully run on WSL2 using the WSL2-enabled Docker Kubernetes or Azure
Kubernetes. If you're on WSL1 you may be better off running in a full Linux VM
under Hyper-V.

### Building on Windows

We have support for building on Windows using PowerShell:

```console
$ cargo build --no-default-features --features rustls-tls
```

**NOTE:** Windows builds use the `rustls` library, which means there are some
things to be aware of. See the [caveats](#caveats) section for more details

## Running

The included example with Krator is the [moose example](/krator/examples).

```
cargo run --example moose
```

## Testing

Krator contains both unit tests and doc tests, and is routinely checked using
`clippy` and `rustfmt` as well.

For unit tests:

```console
$ cargo test --workspace
```

For doc tests:

```console
$ cargo test --doc --all
```

For `clippy`:

```console
$ cargo clippy --workspace
```

For `rustfmt`:

```console
cargo fmt --all -- --check
```

## Creating your own Operators with Krator

If you want to create your own Operator based on Krator, all you need to do is
implement an `Operator`. See the [moose example](/krator/examples/moose.rs) to
see how this is done.
