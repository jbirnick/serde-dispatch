# serde-dispatch

A minimalistic macro for doing RPC (remote procedure calls) using [serde](https://serde.rs/).

When you know Rust then you probably know about static dispatch and dynamic dispatch, and now there is also serde dispatch!
The idea is that:
- a client encodes a request to call a function using some serde format (in this case [bincode](https://docs.rs/bincode/latest/bincode/))
- sends it to the server (in this case through anything that implements `std::io::Write` and `std::io::Read` respectively)
- the server executes the function, encodes the return value, and sends it back to the client

Now this crate allows you to do all of this without writing boilerplate code.
You just:
1. Put `#[serde_dispatch]` on the trait that defines the RPC interface.

   This will:
   - generate all the encoding/decoding boilerplate
   - allow the _server_ to call `handle_with(reader, writer)`
   - allow the _client_ to call `call_with(reader, writer).any_function_of_the_rpc_trait(...)`

2. Provide a means of message transport and expose it through `Write` and `Read` objects.

   You will pass the `Write` and `Read` objects to the client/server interfaces mentioned above.

Please have a look at `example-project/` to see an example usage.

## TODOs (which probably will never be resolved)

- add docs for the main macro
- make the macro generate docs for the items that it generates
- allow various features
  - other encodings
  - async
  - send back confirmation of whether call succeeded (in case when there is no return type)
  - more general measures of transport of the messages
