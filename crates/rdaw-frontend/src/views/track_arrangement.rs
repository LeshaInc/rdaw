use floem::peniko::Color;
use floem::views::{label, Decorators};
use floem::IntoView;
use rdaw_api::{Backend, TrackId};

pub fn track_arrangement<B: Backend>(_id: TrackId, is_even: bool) -> impl IntoView {
    label(move || "Arrangement view...").style(move |s| {
        s.width_full()
            .background(Color::BLACK.with_alpha_factor(if is_even { 0.03 } else { 0.1 }))
    })
}
