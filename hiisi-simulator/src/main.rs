use std::{cell::RefCell, rc::Rc};

use bytes::BytesMut;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use socket2::{Domain, Socket, Type};
use std::os::fd::AsRawFd;

pub struct UserData {
    rng: RefCell<ChaCha8Rng>,
}

type Context = hiisi::server::Context<UserData>;

type IO = hiisi::server::IO<UserData>;

pub fn main() {
    init_logger();

    let seed = match std::env::var("SEED") {
        Ok(seed) => seed.parse::<u64>().unwrap(),
        Err(_) => rand::thread_rng().next_u64(),
    };

    log::info!("Starting simulation with seed {}", seed);

    let rng = ChaCha8Rng::seed_from_u64(seed);
    let user_data = UserData {
        rng: RefCell::new(rng),
    };
    let ctx = Context::new(Rc::new(hiisi::manager::ResourceManager::new()), user_data);
    let mut io = hiisi::server::IO::new(ctx);

    let server_addr: std::net::SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let server_sock = Rc::new(Socket::new(Domain::IPV4, Type::STREAM, None).unwrap());
    let client_sock = Rc::new(Socket::new(Domain::IPV4, Type::STREAM, None).unwrap());

    // Bind the server socket to the server address.
    hiisi::server::serve(&mut io, server_sock, server_addr.clone().into());

    // Connect the client socket to the server address.
    io.connect(client_sock, server_addr.clone().into(), on_client_connect);

    // Main simulation loop.
    loop {
        io.run_once();
    }
}

fn on_client_connect(io: &mut IO, sock: Rc<socket2::Socket>, client_addr: socket2::SockAddr) {
    let sockfd = sock.as_raw_fd();
    log::trace!("Client is connected to {}", sockfd);
    perform_client_req(io, sock);
}

enum PerformClientReqFault {
    // Client sends an empty message to the server.
    Empty,
}

fn gen_perform_client_req_fault(
    ctx: &hiisi::server::Context<UserData>,
) -> Option<PerformClientReqFault> {
    let user_data = &ctx.user_data;
    let mut rng = user_data.rng.borrow_mut();
    if rng.gen_bool(0.1) {
        Some(PerformClientReqFault::Empty)
    } else {
        None
    }
}

fn perform_client_req(io: &mut IO, sock: Rc<Socket>) {
    let ctx = io.context();
    match gen_perform_client_req_fault(ctx) {
        Some(PerformClientReqFault::Empty) => {
            perform_client_req_empty(io, sock);
        }
        None => {
            perform_client_req_normal(io, sock);
        }
    }
}

fn perform_client_req_empty(io: &mut IO, sock: Rc<Socket>) {
    let http_req = BytesMut::from("");
    io.send(sock, http_req.into(), 0, on_client_send);
}

fn perform_client_req_normal(io: &mut IO, sock: Rc<Socket>) {
    let req = hiisi::proto::PipelineReqBody {
        baton: None,
        requests: vec![],
    };
    let buf = hiisi::proto::format_msg(&req).unwrap();
    let mut http_req = BytesMut::new();
    let http_header = format!("POST / HTTP/1.1\r\nContent-Length: {}\r\n\r\n", buf.len());
    http_req.extend_from_slice(http_header.as_bytes());
    http_req.extend_from_slice(&buf);
    let n = http_req.len();
    io.send(sock, http_req.into(), n, on_client_send);
}

fn on_client_send(io: &mut IO, server_sock: Rc<socket2::Socket>, n: usize) {
    io.recv(server_sock, on_client_recv);
}

fn on_client_recv(io: &mut IO, socket: Rc<socket2::Socket>, buf: &[u8], n: usize) {
    perform_client_req(io, socket);
}

fn init_logger() {
    let env = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(env).init();
}
