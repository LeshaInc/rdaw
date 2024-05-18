use futures_lite::future::block_on;

fn main() {
    tracing_subscriber::fmt::init();

    let backend = rdaw_backend::Backend::new();
    let handle = backend.handle();

    std::thread::spawn(move || block_on(backend.run()));

    rdaw_frontend::run(handle);
}
