use std::sync::Arc;
use std::thread;

use futures::executor::block_on;
use rdaw_backend::Backend;
use rdaw_rpc::{transport, Client};
use tracing_error::ErrorLayer;
use tracing_subscriber::layer::SubscriberExt;

fn main() {
    let subscriber = tracing_subscriber::fmt()
        .finish()
        .with(ErrorLayer::default());

    tracing::subscriber::set_global_default(subscriber).unwrap();

    let (client_transport, server_transport) = transport::local(None);

    let mut backend = Backend::new(server_transport);
    thread::spawn(move || block_on(backend.handle()).unwrap());

    let client = Client::new(client_transport);

    let client_clone = client.clone();
    thread::spawn(move || block_on(client_clone.handle()).unwrap());

    rdaw_frontend::run(Arc::new(client));
}
