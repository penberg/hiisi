use bytes::Bytes;

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::os::fd::AsRawFd;
use std::pin::Pin;
use std::rc::Rc;

struct Socket {
    local_sock: Rc<socket2::Socket>,
    remote_sock: Rc<socket2::Socket>,
    xmit_queue: RefCell<VecDeque<Bytes>>,
}

pub struct IO<C> {
    context: C,
    completions: RefCell<VecDeque<Completion<C>>>,
    listener_sockets: HashMap<i32, Rc<socket2::Socket>>,
    conn_sockets: HashMap<i32, Socket>,
    accept_listeners: HashMap<socket2::SockAddr, (Rc<socket2::Socket>, AcceptCallback<C>)>,
    recv_listeners: HashMap<i32, (Rc<socket2::Socket>, RecvCallback<C>)>,
}

impl<C> IO<C> {
    pub fn new(context: C) -> Self {
        let completions = RefCell::new(VecDeque::new());
        let listener_sockets = HashMap::new();
        let conn_sockets = HashMap::new();
        let accept_listeners = HashMap::new();
        let recv_listeners = HashMap::new();
        Self {
            context,
            completions,
            listener_sockets,
            conn_sockets,
            accept_listeners,
            recv_listeners,
        }
    }

    pub fn context(&self) -> &C {
        &self.context
    }

    pub fn block_on<T>(&self, mut future: impl Future<Output = T>) -> T {
        let waker = futures::task::noop_waker();
        let cx = &mut std::task::Context::from_waker(&waker);
        let mut future = unsafe { Pin::new_unchecked(&mut future) };
        loop {
            match future.as_mut().poll(cx) {
                std::task::Poll::Ready(val) => break val,
                std::task::Poll::Pending => {
                    // TODO: We could use a background task and call a completion here.
                }
            }
        }
    }

    pub fn run_once(&mut self) {
        self.flush_xmit_queues();
        self.flush_completions();
    }

    fn flush_xmit_queues(&mut self) {
        let mut completions = Vec::new();
        for (sockfd, socket) in self.conn_sockets.iter_mut() {
            let mut xmit_queue = socket.xmit_queue.borrow_mut();
            if xmit_queue.is_empty() {
                continue;
            }
            let remote_sockfd = socket.remote_sock.as_raw_fd();
            if !self.recv_listeners.contains_key(&remote_sockfd) {
                continue;
            }
            let local_sockfd = socket.local_sock.as_raw_fd();
            log::trace!(
                "IO -> flush_xmit_queues(local_sockfd={}, remote_sockfd={})",
                local_sockfd,
                remote_sockfd
            );
            let (recv_socket, cb) = self.recv_listeners.remove(&remote_sockfd).unwrap();
            while let Some(buf) = xmit_queue.pop_front() {
                let c = Completion::Recv {
                    sock: recv_socket.clone(),
                    buf,
                    cb,
                };
                completions.push(c);
            }
        }
        for c in completions {
            self.enqueue(c);
        }
    }

    fn flush_completions(&mut self) {
        let mut completions: Vec<Completion<C>> = self.completions.borrow_mut().drain(..).collect();
        loop {
            let c = match completions.pop() {
                Some(c) => c,
                None => break,
            };
            c.complete(self);
        }
    }

    pub fn connect(
        &mut self,
        local_sock: Rc<socket2::Socket>,
        remote_addr: socket2::SockAddr,
        cb: ConnectCallback<C>,
    ) {
        let local_sockfd = local_sock.as_raw_fd();
        log::trace!(
            "IO -> connect(sockfd={}, addr={:?})",
            local_sockfd,
            remote_addr
        );

        // Bind the local socket to a random port.
        let local_port = self.conn_sockets.len() as u16 + 30000;
        let local_addr = format!("127.0.0.1:{}", local_port);
        let local_addr: std::net::SocketAddr = local_addr.parse().unwrap();
        let local_addr: socket2::SockAddr = local_addr.into();

        // Accept the connection by creating a new socket on the remote side.
        let (accept_sock, accept_cb) = self.accept_listeners.remove(&remote_addr).unwrap();
        let remote_sock = Rc::new(
            socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::STREAM, None).unwrap(),
        );
        self.register_socket(remote_sock.clone(), local_sock.clone());
        let c = Completion::Accept {
            server_sock: accept_sock.clone(),
            server_addr: remote_addr.clone(),
            client_sock: remote_sock.clone(),
            client_addr: local_addr.clone(),
            cb: accept_cb,
        };
        self.enqueue(c);

        // Establish the connection by registering the local socket.
        self.register_socket(local_sock.clone(), remote_sock.clone());
        let c = Completion::Connect {
            sock: local_sock.clone(),
            addr: local_addr,
            cb,
        };
        self.enqueue(c);
    }

    fn register_socket(
        &mut self,
        local_sock: Rc<socket2::Socket>,
        remote_sock: Rc<socket2::Socket>,
    ) {
        let local_sockfd = local_sock.as_raw_fd();
        let remote_sockfd = remote_sock.as_raw_fd();
        log::trace!(
            "IO -> register_socket(local_sock={}, remote_sock={})",
            local_sockfd,
            remote_sockfd
        );
        self.conn_sockets.insert(
            local_sockfd,
            Socket {
                local_sock: local_sock.clone(),
                remote_sock: remote_sock.clone(),
                xmit_queue: RefCell::new(VecDeque::new()),
            },
        );
    }

    pub fn accept(
        &mut self,
        server_sock: Rc<socket2::Socket>,
        addr: socket2::SockAddr,
        cb: AcceptCallback<C>,
    ) {
        let sockfd = server_sock.as_raw_fd();
        log::trace!("IO -> accept(sockfd={})", sockfd);
        self.listener_sockets.insert(sockfd, server_sock.clone());
        self.accept_listeners.insert(addr, (server_sock, cb));
    }

    pub fn close(&mut self, sock: Rc<socket2::Socket>) {
        let sockfd = sock.as_raw_fd();
        log::trace!("IO -> close(sockfd={})", sockfd);
        self.conn_sockets.remove(&sockfd);
    }

    pub fn recv(&mut self, sock: Rc<socket2::Socket>, cb: RecvCallback<C>) {
        let sockfd = sock.as_raw_fd();
        log::trace!("IO -> recv(sockfd={})", sockfd);
        self.recv_listeners.insert(sockfd, (sock, cb));
    }

    pub fn send(&mut self, sock: Rc<socket2::Socket>, buf: Bytes, n: usize, cb: SendCallback<C>) {
        let sockfd = sock.as_raw_fd();
        log::trace!("IO -> send(sockfd={})", sockfd);
        let socket = self.conn_sockets.get(&sockfd).unwrap();
        let localfd = socket.local_sock.as_raw_fd();
        assert!(localfd == sockfd);
        socket.xmit_queue.borrow_mut().push_back(buf.clone());
        let c = Completion::Send { sock, buf, n, cb };
        self.enqueue(c);
    }

    fn enqueue(&self, c: Completion<C>) {
        let mut completions = self.completions.borrow_mut();
        completions.push_back(c);
    }
}

pub enum Completion<C> {
    Connect {
        sock: Rc<socket2::Socket>,
        addr: socket2::SockAddr,
        cb: ConnectCallback<C>,
    },
    Accept {
        server_sock: Rc<socket2::Socket>,
        server_addr: socket2::SockAddr,
        client_sock: Rc<socket2::Socket>,
        client_addr: socket2::SockAddr,
        cb: AcceptCallback<C>,
    },
    Close,
    Recv {
        sock: Rc<socket2::Socket>,
        buf: Bytes,
        cb: RecvCallback<C>,
    },
    Send {
        sock: Rc<socket2::Socket>,
        buf: Bytes,
        n: usize,
        cb: SendCallback<C>,
    },
}

impl<C> std::fmt::Debug for Completion<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Completion::Connect { .. } => write!(f, "Connect"),
            Completion::Accept { .. } => write!(f, "Accept"),
            Completion::Close => write!(f, "Close"),
            Completion::Recv { .. } => write!(f, "Recv"),
            Completion::Send { .. } => write!(f, "Send"),
        }
    }
}

impl<C> Completion<C> {
    fn key(&self) -> usize {
        match self {
            Completion::Connect { sock, .. } => sock.as_raw_fd() as usize,
            Completion::Accept { server_sock, .. } => server_sock.as_raw_fd() as usize,
            Completion::Close => todo!(),
            Completion::Recv { sock, .. } => sock.as_raw_fd() as usize,
            Completion::Send { sock, .. } => sock.as_raw_fd() as usize,
        }
    }

    fn prepare(&self) {
        match self {
            Completion::Connect { .. } => {}
            Completion::Accept { .. } => {}
            Completion::Close => {
                todo!();
            }
            Completion::Recv { .. } => {}
            Completion::Send { .. } => {}
        }
    }

    fn complete(self, io: &mut IO<C>) {
        match self {
            Completion::Connect { sock, addr, cb } => {
                cb(io, sock, addr);
            }
            Completion::Accept {
                server_sock,
                server_addr,
                client_sock,
                client_addr,
                cb,
            } => {
                cb(io, server_sock, server_addr, client_sock, client_addr);
            }
            Completion::Close => {
                todo!();
            }
            Completion::Recv { sock, buf, cb } => {
                let n = buf.len();
                cb(io, sock, &buf, n);
            }
            Completion::Send { sock, buf, n, cb } => {
                cb(io, sock, n);
            }
        }
    }
}

type ConnectCallback<C> = fn(&mut IO<C>, Rc<socket2::Socket>, socket2::SockAddr);

type AcceptCallback<C> =
    fn(&mut IO<C>, Rc<socket2::Socket>, socket2::SockAddr, Rc<socket2::Socket>, socket2::SockAddr);

type RecvCallback<C> = fn(&mut IO<C>, Rc<socket2::Socket>, &[u8], usize);

type SendCallback<C> = fn(&mut IO<C>, Rc<socket2::Socket>, usize);
