use anyhow::Result;
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

fn execute_request<T>(io: &mut IO<T>, buf: &[u8]) -> Result<Bytes> {
    let ctx = io.context();
    let req = parse_request(&buf)?;
    let resp = executor::execute_client_req(ctx.manager.clone(), req)?;
    Ok(proto::format_msg(&resp)?)
}

fn on_recv<T>(io: &mut IO<T>, sock: Rc<Socket>, buf: &[u8], n: usize) {
    if n == 0 {
        log::trace!("Client closed connection");
        io.close(sock);
        return;
    }
    let resp = match execute_request(io, &buf[..n]) {
        Ok(resp) => format_response(resp, http::StatusCode::OK),
        Err(x) => format_response(
            format!("{}", x).into(),
            http::StatusCode::INTERNAL_SERVER_ERROR,
        ),
    };

    let n = resp.len();
    io.send(sock, resp.into(), n, on_send);
}

fn parse_request(buf: &[u8]) -> Result<Request> {
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Request::new(&mut headers);
    let body_off = req.parse(buf).unwrap().unwrap();
    let database = "hello"; // TODO: take from request path
    let req = proto::parse_client_req(&buf[body_off..])?;
    Ok(Request {
        database: database.to_owned(),
        req,
    })
}

fn format_response(body: Bytes, status: http::StatusCode) -> Bytes {
    let n = body.len();

    let response = http::Response::builder().status(status).body(body).unwrap();

    let mut response_bytes = BytesMut::new();
    response_bytes.extend_from_slice(
        format!(
            "HTTP/1.1 {} {}\r\n",
            response.status().as_u16(),
            response.status().canonical_reason().unwrap_or("")
        )
        .as_bytes(),
    );

    for (key, value) in response.headers() {
        response_bytes.extend_from_slice(
            format!("{}: {}\r\n", key.as_str(), value.to_str().unwrap()).as_bytes(),
        );
    }

    response_bytes.extend_from_slice(format!("Content-Length: {}\r\n\r\n", n).as_bytes());
    response_bytes.extend_from_slice(response.into_body().as_ref());

    response_bytes.into()
}

fn on_send<T>(io: &mut IO<T>, sock: Rc<Socket>, _n: usize) {
    io.recv(sock, on_recv)
}
