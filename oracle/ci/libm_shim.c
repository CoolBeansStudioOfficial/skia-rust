/* Diagnostic only (oracle/README.md, "CI-hosted oracle"): LD_PRELOAD shim that makes glibc's
 * float math functions behave like "compute in double, round to float", which is how the
 * Windows UCRT implements most of them. If the GMs that differ from the Windows goldens become
 * byte-identical under this shim, the difference is libm. Not used for goldens. */
#include <math.h>

#define F1(f) float f##f(float x) { return (float)f((double)x); }
#define F2(f) float f##f(float x, float y) { return (float)f((double)x, (double)y); }

float sinf(float x) { return (float)sin((double)x); }
float cosf(float x) { return (float)cos((double)x); }
float tanf(float x) { return (float)tan((double)x); }
float asinf(float x) { return (float)asin((double)x); }
float acosf(float x) { return (float)acos((double)x); }
float atanf(float x) { return (float)atan((double)x); }
float atan2f(float y, float x) { return (float)atan2((double)y, (double)x); }
float sinhf(float x) { return (float)sinh((double)x); }
float coshf(float x) { return (float)cosh((double)x); }
float tanhf(float x) { return (float)tanh((double)x); }
float expf(float x) { return (float)exp((double)x); }
float exp2f(float x) { return (float)exp2((double)x); }
float logf(float x) { return (float)log((double)x); }
float log2f(float x) { return (float)log2((double)x); }
float log10f(float x) { return (float)log10((double)x); }
float powf(float x, float y) { return (float)pow((double)x, (double)y); }
float cbrtf(float x) { return (float)cbrt((double)x); }
float hypotf(float x, float y) { return (float)hypot((double)x, (double)y); }
void sincosf(float x, float* s, float* c) { *s = (float)sin((double)x); *c = (float)cos((double)x); }
