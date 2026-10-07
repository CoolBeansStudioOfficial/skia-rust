/*
 * skia-rust oracle: raw output dump for DM. Test tooling only; never shipped.
 *
 * Copied into third_party/skia/dm/ by `cargo xtask oracle patch`.
 */

#ifndef OracleDump_DEFINED
#define OracleDump_DEFINED

#include <cstddef>

class SkBitmap;
class SkString;

namespace OracleDump {

// True when --oracleRawPath was passed.
bool Enabled();

// Writes one DM result as <oracleRawPath>/<sink>/<src>/<options>/<name>.{raw|bin,json}.
// `bitmap` holds raster results; `data`/`len` hold encoded results (PDF, SVG, SKP), which
// are written as-is to a .bin file.
void Write(const SkString& sinkTag,
           const SkString& srcTag,
           const SkString& srcOptions,
           const SkString& name,
           const SkBitmap& bitmap,
           const void* data,
           size_t len);

}  // namespace OracleDump

#endif
