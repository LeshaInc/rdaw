use std::sync::Arc;
use std::thread;

use futures::executor::block_on;
use rdaw_backend::Backend;
use rdaw_rpc::{transport, Client};
use tracing_error::ErrorLayer;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter, Layer};

fn main() {
    let subscriber = fmt::layer()
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_filter(EnvFilter::from_default_env());

    tracing_subscriber::registry()
        .with(subscriber)
        .with(ErrorLayer::default())
        .init();

    let (client_transport, server_transport) = transport::local(None);

    let mut backend = Backend::new(server_transport);
    thread::spawn(move || block_on(backend.handle()).unwrap());

    let client = Client::new(client_transport);

    let client_clone = client.clone();
    thread::spawn(move || block_on(client_clone.handle()).unwrap());

    rdaw_frontend::run(Arc::new(client));
}
