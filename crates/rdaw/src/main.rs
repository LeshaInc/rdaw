use std::thread;

use futures_lite::future::block_on;
use rdaw_backend::Backend;
use rdaw_rpc::{transport, Client};

fn main() {
    tracing_subscriber::fmt::init();

    let (client_transport, server_transport) = transport::local(None);

    let mut backend = Backend::new(server_transport);
    thread::spawn(move || block_on(backend.handle()).unwrap());

    let client = Client::new(client_transport);

    let client_clone = client.clone();
    thread::spawn(move || block_on(client_clone.handle()).unwrap());

    rdaw_frontend::run(client);
}
