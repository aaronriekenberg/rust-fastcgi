# rust-fastcgi

## What is this?

A port of a go FastCGI app (https://github.com/aaronriekenberg/go-fastcgi) into Rust.

Mostly this an exercise to learn more Rust.

Currently this is the backend for https://aaronr.digital on a Raspberry Pi.

This app provides a small REST style api with these JSON endpoints:

* `/cgi-bin/request_info` echo details about the current request.
* `/cgi-bin/commands` return a list of configured commands that can be run.
* `/cgi-bin/commands/<command_id>` run a command and return the result as a JSON response.

## How do I run this?

* Install Rust (https://www.rust-lang.org/)
* Clone this git repo
* `cargo build -v` build in debug mode (fast build, not optimized)
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
* [tokio-fastcgi](https://github.com/FlashSystems/tokio-fastcgi) excellent library depending on tokio that implements the fastcgi protocol. 
  * This app implements a FastCGI "Responder" (aka server)
* [async-trait](https://github.com/dtolnay/async-trait) allows rust traits to contain `async` functions
* [env_logger](https://github.com/env-logger-rs/env_logger) logger configurable via environment variables
* Error handling - learning 2 elegant libraries by the same author:
  * [anyhow](https://github.com/dtolnay/anyhow) used for application error handling to propogate and format fatal errors.
  * [thiserror](https://github.com/dtolnay/thiserror) used for defining custom error types.  Used for internal APIs that need precise error handling.
* [serde](https://serde.rs/) used for marshalling and unmarshalling JSON.


## Some learnings:

* Using `anyhow` and `thiserror` for error handling as opposed to `Box<dyn Error>`.  I like this and begin to understand the use case for 2 separate libraries.
* Traits having dynamic dispatch and async functions.  `handlers::RequestHandler` is similar to `http.Handler` in go.
* Generic lifetime parameters in structs to avoid data copies.  See `request::FastCGIRequest` and `handlers::request_info::RequestInfoResponse` for examples.
* Use of generics in server code to allow common connection and request processing code while supporting both tcp and unix server socket types.
* Use of `'static` lifetime in `server::ConnectionProcessor::handle_connection` to allow for owned but non-global types.  [This reference](https://github.com/pretzelhammer/rust-blog/blob/master/posts/common-rust-lifetime-misconceptions.md#2-if-t-static-then-t-must-be-valid-for-the-entire-program) was very enlightning.
* `handlers::command::RunCommandResponse` contains perhaps the first 128-bit variable I have ever used :)
