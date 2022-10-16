# rust-fastcgi

## What is this?

A port of a go FastCGI app (https://github.com/aaronriekenberg/go-fastcgi) into Rust.

Mostly this an exercise to learn more Rust.

Currently this is the backend for https://aaronr.digital.

This app provides a small REST style api with these JSON endpoints:

* `/cgi-bin/request_info` echo details about the current request.
* `/cgi-bin/commands` return a list of configured commands that can be run.
* `/cgi-bin/commands/<command_id>` run a command and return the result as a JSON response.

## How do I run this?

* Install Rust (https://www.rust-lang.org/)
* Clone this git repo
* `cargo build -v`
* Run the app setting logging level to debug and using configuration file `./config/unix.json`:

```
RUST_LOG=debug ./target/debug/rust-fastcgi ./config/unix.json
```

## What is the tech stack?

* [tokio](https://tokio.rs/) asynchronous runtime for rust.  From tokio this app uses:
  * `async` / `await` functions (aka coroutines)
  * TCP and UNIX server sockets
  * Timeouts
  * Semaphores
* [tokio-fastcgi](https://github.com/FlashSystems/tokio-fastcgi) library depending on tokio that implements the fastcgi protocol. 
  * This app implements a FastCGI "Responder" (aka server)
* [async-trait](https://github.com/dtolnay/async-trait) allows rust traits to contain `async` functions
* [env_logger](https://github.com/env-logger-rs/env_logger) logger configurable via environment variables
* Error handling - learning 2 elegant libraries by the same author:
  * [anyhow](https://github.com/dtolnay/anyhow) used for application error handling to propogate and format fatal errors.
  * [thiserror](https://github.com/dtolnay/thiserror) used for defining custom error types.  Used for internal APIs that need precise error handling.
* [serde](https://serde.rs/) used for marshalling and unmarshalling JSON.
