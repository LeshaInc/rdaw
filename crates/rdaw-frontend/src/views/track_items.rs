use floem::peniko::Color;
use floem::views::{label, Decorators};
use floem::IntoView;
use rdaw_api::track::TrackId;
use rdaw_api::Backend;

pub fn track_items<B: Backend>(_id: TrackId, is_even: bool) -> impl IntoView {
    label(move || "Track items...").style(move |s| {
        s.width_full()
            .background(Color::BLACK.with_alpha_factor(if is_even { 0.03 } else { 0.1 }))
    })
}
