# gm/pathreverse.cpp::pathreverse

Status: passing (`tests/gm/src/gm/pathreverse.rs`), every tier.

The earlier mismatch (4955 px in the moveTo/lineTo + addOval block) was recorded for an attempt whose
port was not registered on the branch. This attempt wrote the port from `gm/pathreverse.cpp` and it
matches the golden on the first run, with `SkPathPriv::ReverseAddPath` ported as
`path_priv::reverse_add_path` (`PathBuilder::private_reverse_add_path`, compared line by line with
`SkPathBuilder.cpp#L970-L1019`). No code change was needed; the reversed open contour and the conic
ovals reverse correctly. If the old mismatch reappears, first check that the port matches the
manifest source range `gm/pathreverse.cpp#L75-L97`.
