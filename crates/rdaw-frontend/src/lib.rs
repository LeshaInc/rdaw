use futures_lite::future::block_on;
use rdaw_api::Operations;

pub fn run<B: Operations>(backend: B) {
    block_on(async move {
        let track = backend.create_track("Unnamed".into()).await.unwrap();
        backend.set_track_name(track, "Test".into()).await.unwrap();
        assert_eq!(backend.get_track_name(track).await.unwrap(), "Test");
    });
}
