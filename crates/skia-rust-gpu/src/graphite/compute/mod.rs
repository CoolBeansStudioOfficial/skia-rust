//! Ports of `src/gpu/graphite/compute/*`.
//!
//! Only the part the compute pipelines need so far (G11b): `ComputeStep`'s description of its
//! resources and shader, and the `ComputeTypes.h` it names. `DispatchGroup` and the rest of
//! `ComputeStep` (the buffer preparation hooks) come with the compute wave, G13.

pub mod compute_step;
