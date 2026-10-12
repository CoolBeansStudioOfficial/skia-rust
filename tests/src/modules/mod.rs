//! Ports of `modules/<m>/tests`, one module per module and Skia file (see `lib.rs`).

#[cfg(test)]
pub mod skottie;
#[cfg(test)]
#[macro_use]
pub mod skparagraph;
#[cfg(test)]
pub mod sksg;
#[cfg(test)]
pub mod skshaper;
#[cfg(test)]
pub mod skunicode;
