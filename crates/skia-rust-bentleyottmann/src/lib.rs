//! Line segment intersection of skia-rust (`bentleyottmann`), ported from Skia's
//! `modules/bentleyottmann`: the exact integer Bentley-Ottmann sweep, the Myers sweep, a brute
//! force reference, and the contour extraction from paths that feeds them.
//!
//! Each Skia namespace is a Rust module: `bentleyottmann::Point` is [`point::Point`],
//! `myers::Segment` is [`myers::Segment`] and `contour::Contours` is [`contour::Contours`].
//!
//! Integer arithmetic follows C++: the widths are the same (`int32_t` coordinates, `int64_t`
//! cross products, [`int96::Int96`] for products that need 96 bits), and where C++ wraps in
//! practice the port uses `wrapping_*` operations instead of panicking on overflow.

pub mod bentley_ottmann1;
pub mod brute_force_crossings;
pub mod contour;
pub mod event_queue;
pub mod int96;
pub mod myers;
pub mod point;
pub mod segment;
pub mod sweep_line;

pub use event_queue::{EventQueue, EventQueueInterface, SweepLineInterface};
pub use int96::Int96;
pub use point::Point;
pub use segment::{Crossing, Segment};
pub use sweep_line::SweepLine;
