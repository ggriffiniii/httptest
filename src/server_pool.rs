use crate::Server;
use once_cell::sync::OnceCell;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;

/// A pool of shared servers.
///
/// The typical way to use this library is to create a new Server for each test
/// using [Server::run](struct.Server.html#method.run). This way each test
/// contains it's own independent state. However for very large test suites it
/// may be beneficial to share servers between test runs. This is typically only
/// desirable when running into limits because the test framework spins up an
/// independent thread for each test and system wide resources (TCP ports) may
/// become scarce. In those cases you can opt into using a shared ServerPool that
/// will create a maximum of N servers that tests can share. Invoking
/// [get_server](#method.get_server) on the pool will return a
/// [ServerHandle](struct.ServerHandle.html) that deref's to a
/// [Server](struct.Server.html). For the life of the
/// [ServerHandle](struct.ServerHandle.html) it has unique access to this server
/// instance. When the handle is dropped the server expectations are asserted and
/// cleared and the server is returned back into the
/// [ServerPool](struct.ServerPool.html) for use by another test.
///
/// Example:
///
/// ```
/// # use httptest::ServerPool;
/// // Create a server pool that will create at most 99 servers.
/// static SERVER_POOL: ServerPool = ServerPool::new(99);
///
/// #[test]
/// fn test_one() {
///     let server = SERVER_POOL.get_server();
///     server.expect(Expectation::matching(any()).respond_with(status_code(200)));
///     // invoke http requests to server.
///
///     // server will assert expectations are met on drop.
/// }
///
/// #[test]
/// fn test_two() {
///     let server = SERVER_POOL.get_server();
///     server.expect(Expectation::matching(any()).respond_with(status_code(200)));
///     // invoke http requests to server.
///
///     // server will assert expectations are met on drop.
/// }
/// ```

/// A pool of running servers.
#[derive(Debug)]
pub struct ServerPool(OnceCell<InnerPool>, usize);

impl ServerPool {
    /// Create a new pool of servers.
    ///
    /// `max_servers` is the maximum number of servers that will be created.
    /// servers are created on-demand when `get_server` is invoked.
    pub const fn new(max_servers: usize) -> Self {
        ServerPool(OnceCell::new(), max_servers)
    }

    /// Get the next available server from the pool.
    pub fn get_server(&self) -> ServerHandle {
        self.0.get_or_init(|| InnerPool::new(self.1)).get_server()
    }
}

#[allow(clippy::mutex_atomic)]
#[derive(Debug)]
struct InnerPool {
    servers_created: Mutex<usize>,
    servers_tx: crossbeam_channel::Sender<Server>,
    servers_rx: crossbeam_channel::Receiver<Server>,
}

#[allow(clippy::mutex_atomic)]
impl InnerPool {
    fn new(max_capacity: usize) -> Self {
        assert!(max_capacity > 0);
        let (servers_tx, servers_rx) = crossbeam_channel::bounded(max_capacity);
        InnerPool {
            servers_created: Mutex::new(0),
            servers_tx,
            servers_rx,
        }
    }

    fn get_server(&self) -> ServerHandle {
        if let Ok(server) = self.servers_rx.try_recv() {
            return ServerHandle {
                servers_tx: self.servers_tx.clone(),
                server: Some(server),
                lifetime_marker: PhantomData,
            };
        }
        {
            let mut servers_created = self.servers_created.lock().expect("poisoned mutex");
            if *servers_created < self.servers_tx.capacity().unwrap() {
                *servers_created += 1;
                return ServerHandle {
                    servers_tx: self.servers_tx.clone(),
                    server: Some(Server::run()),
                    lifetime_marker: PhantomData,
                };
            }
        }
        ServerHandle {
            servers_tx: self.servers_tx.clone(),
            server: Some(
                self.servers_rx
                    .recv()
                    .expect("all senders unexpectedly dropped"),
            ),
            lifetime_marker: PhantomData,
        }
    }
}

#[allow(clippy::mutex_atomic)]
impl Drop for InnerPool {
    fn drop(&mut self) {
        // wait for all created servers to get returned to the pool.
        let servers_created = self.servers_created.lock().expect("poisoned mutex");
        for _ in 0..*servers_created {
            self.servers_rx
                .recv()
                .expect("all senders unexpectedly dropped");
        }
    }
}

/// A handle to a server. Expectations are inserted when the handle is dropped.
#[derive(Debug)]
pub struct ServerHandle<'a> {
    servers_tx: crossbeam_channel::Sender<Server>,
    server: Option<Server>,

    // We add a lifetime to the ServerHandle just to restrict the ownership
    // beyond what the implementation currently allows. The public facing API
    // will appear that the ServerHandle is borrowed from a Pool, which enforces
    // the desired behavior. No SeverHandle should outlive the underlying Pool.
    // The current implementation passes around owned values so there is no real
    // borrowing constraint, but a future implementation may do something more
    // efficient.
    lifetime_marker: PhantomData<&'a ()>,
}

impl Deref for ServerHandle<'_> {
    type Target = Server;

    fn deref(&self) -> &Server {
        self.server.as_ref().unwrap()
    }
}

impl DerefMut for ServerHandle<'_> {
    fn deref_mut(&mut self) -> &mut Server {
        self.server.as_mut().unwrap()
    }
}

impl Drop for ServerHandle<'_> {
    fn drop(&mut self) {
        let mut server = self.server.take().unwrap();
        server.verify_and_clear();
        self.servers_tx
            .send(server)
            .expect("all receivers unexpectedly dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_SERVERS: usize = 5;
    static POOL: ServerPool = ServerPool::new(MAX_SERVERS);

    #[test]
    fn test_max_threads() {
        use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
        let concurrent_server_handles = AtomicUsize::new(0);
        let desired_concurrency_reached = std::sync::Barrier::new(MAX_SERVERS);
        crossbeam_utils::thread::scope(|s| {
            for _ in 0..10 {
                s.spawn(|_| {
                    let _server = POOL.get_server();

                    // Ensure that we've reached the desired number of concurrent servers.
                    desired_concurrency_reached.wait();

                    // Ensure that we have not exceeded the desired number of concurrent servers.
                    let prev_value = concurrent_server_handles.fetch_add(1, SeqCst);
                    if prev_value > MAX_SERVERS {
                        panic!("too many concurrent server handles: {}", prev_value + 1);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    concurrent_server_handles.fetch_sub(1, SeqCst);
                });
            }
        })
        .unwrap();
    }
}
