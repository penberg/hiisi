# Architecture

Hiisi is an alternative to the libSQL server, which allows clients in
serverless environments to execute SQL against a libSQL/SQLite database
remotely. The Hiisi server architecture follows similar principles as
TigerBeetle to provide deterministic simulation testing support.

## IO dispatcher

The I/O dispatcher is the heart of the architecture, inspired by
[TigerBeetle's I/O dispatch] and [Mitchell Hashimoto's libxev]. The I/O
dispatcher supports operation such as `accept()`, `recvmsg()`, and
`sendmsg()`. However, instead of executing the operations immediately, the I/O
dispatcher executes them asynchronously. The caller of the operations passes a
callback function, which the I/O dispatcher executes when an operation
completes. The I/O dispatcher also has a `run_once()` method, which runs the
I/O dispatch once.

As an example, let's consider a TCP echo server. The top-level server function
looks like this:

```
fn main() {
 let sock = listen();

 let mut io = IO::new(());

 io.accept(sock, addr, on_accept);
 loop {
 io.run_once();
 }
}

fn listen() -> Rc<Socket> {
 // ...
}
```

The application creates a listener socket with the `listen()` function
(details omitted for brevity), then creates the I/O dispatcher object,
executes an `accept()` operation on the listener socket, and then executes the
I/O dispatcher in a loop.

The `on_accept` accept handler that accepts new connections looks like this:

```
fn on_accept<T>(
 io: &mut IO<T>,
 server_sock: Rc<Socket>,
 server_addr: SockAddr,
 conn_sock: Rc<Socket>,
 sock_addr: SockAddr,
) {
 io.accept(server_sock, server_addr, on_accept);
 io.recv(conn_sock, on_recv);
}
```

The handler's first step is to execute an `accept()` operation again on the
listener socket. We do that because we need to explicitly instruct the I/O
dispatcher that we want to accept more connections. The second step is to
perform the `recv()` operation, which receives messages from the newly
established client connection.

The receive handler function is simple: we send back the message we received:

```
fn on_recv<T>(io: &mut IO<T>, sock: Rc<Socket>, buf: &[u8], n: usize) {
 io.send(sock, buf, n, on_send);
}
```

Finally, the send handler, which is called when we have sent the message,
performs a `recv()` operation again to wait for the following message arriving
on the connection:

```
fn on_send<T>(io: &mut IO<T>, sock: Rc<Socket>, _n: usize) {
 io.recv(sock, on_recv)
}
```

## Server

The server performs SQL statements on libSQL/SQLite databases that it manages
on behalf of a client. The server implements the [libSQL wire protocol],
similar to PostgreSQL or MySQL write protocols in functionality but layered on
top of HTTP. When a client sends an HTTP request to the server, we go through
the accept handler and perform a `recv()` operation. We parse and execute the
request when an HTTP message arrives on the server.

We first need to open a connection to a libSQL/SQLite database to execute a
SQL statement. We keep a fixed number of databases as memory residents to
support transactions that span multiple HTTP requests, which guarantees SQLite
transaction semantics. If we are at the limit for memory resident databases,
we expire the least used connections as per the SIEVE cache replacement
policy.

When a SQL statement is executed, we take the result set and send it back to
the client in serialized form using the `send()` operation, which ends the
HTTP request processing.

## Async support

The I/O dispatcher supports executing async code with the `block_on()`
operation. Right now, futures are executed with busy polling, but in future
work, we will explore more efficient ways to do this.

## Simulator

The simulator builds on the I/O dispatch with a special-purpose implementation
that simulates the operations. When the simulator starts, the user either
specifies a seed number or the simulator generates one. Every simulated step
is determined by a pseudo-random number generator, which guarantees that if
you pass the same seed for two simulation runs, they will perform the same
steps. This means that if your simulation finds a fault, you can reproduce the
fault as long as you know the seed. The simulator is essentially a loop that
generates a client operation and executes the simulated I/O dispatch, which
executes the same server logic you run in production.

[TigerBeetle's I/O dispatch]: https://tigerbeetle.com/blog/a-friendly-abstraction-over-iouring-and-kqueue

[Mitchell Hashimoto's libxev]: https://github.com/mitchellh/libxev

[libSQL wire protocol]: https://github.com/tursodatabase/libsql/blob/main/docs/HRANA_2_SPEC.md
