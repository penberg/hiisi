use bytes::{Bytes, BytesMut};
use socket2::{SockAddr, Socket};

use std::rc::Rc;

use crate::executor::{self, Request};
use crate::proto;
use crate::ResourceManager;

pub type IO<T> = crate::io::IO<Context<T>>;

pub struct Context<T> {
    manager: Rc<ResourceManager>,
    pub user_data: T,
}

impl<T> Context<T> {
    pub fn new(manager: Rc<ResourceManager>, user_data: T) -> Self {
        Self { manager, user_data }
    }
}

pub fn serve<T>(io: &mut IO<T>, sock: Rc<Socket>, addr: SockAddr) {
    io.accept(sock, addr, on_accept);
}

fn on_accept<T>(
    io: &mut IO<T>,
    server_sock: Rc<Socket>,
    server_addr: SockAddr,
    conn_sock: Rc<Socket>,
    sock_addr: SockAddr,
) {
    log::trace!("Server accepted connection from {:?}", sock_addr);
    io.accept(server_sock, server_addr, on_accept);
    io.recv(conn_sock, on_recv);
}

fn on_recv<T>(io: &mut IO<T>, sock: Rc<Socket>, buf: &[u8], n: usize) {
    if n == 0 {
        log::trace!("Client closed connection");
        io.close(sock);
        return;
    }
    let ctx = io.context();
    let req = parse_request(&buf[..n]);
    let resp = io
        .block_on(executor::execute_client_req(ctx.manager.clone(), req))
        .unwrap();
    let resp = format_response(resp);
    let n = resp.len();
    io.send(sock, resp.into(), n, on_send);
}

fn parse_request(buf: &[u8]) -> Request {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);
    let body_off = req.parse(buf).unwrap().unwrap();
    let database = "hello"; // TODO: take from request path
    let req = proto::parse_client_req(&buf[body_off..]).unwrap();
    Request {
        database: database.to_owned(),
        req,
    }
}

fn format_response(resp: proto::PipelineRespBody) -> Bytes {
    let body = proto::format_msg(&resp).unwrap();
    let header = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len());
    let mut ret = BytesMut::new();
    ret.extend_from_slice(header.as_bytes());
    ret.extend_from_slice(&body);
    ret.into()
}

fn on_send<T>(io: &mut IO<T>, sock: Rc<Socket>, _n: usize) {
    io.recv(sock, on_recv)
}
