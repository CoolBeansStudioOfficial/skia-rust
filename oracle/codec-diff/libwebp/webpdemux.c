/*
 * Differential driver for the demuxer of skia-rust-libwebp (docs/design/codecs.md §9.1). For each
 * file it parses the data twice, as WebPDemux (complete) and WebPDemuxPartial (partial), and
 * prints one line per fact, in the order below:
 *
 *   <base> <mode> state=<WebPDemuxState> demux=<0|1> [flags=<f> canvas=<w>x<h> frames=<n> loop=<l>
 *       bgcolor=<hex>]
 *   <base> <mode> frame=<i> x=<x> y=<y> w=<w> h=<h> dur=<d> dispose=<0|1> blend=<0|1>
 *       complete=<0|1> alpha=<0|1> size=<bytes> fnv=<16 hex digits>        (frames by NextFrame)
 *   <base> <mode> last frame=<n>                                            (GetFrame(0))
 *   <base> <mode> chunk=<fourcc> <i>/<n> size=<bytes> fnv=<16 hex digits>   (ICCP, EXIF, XMP)
 *
 * <mode> is "full" or "partial". Frame and chunk payloads are hashed with FNV-1a 64. The
 * blend value is WebPMuxAnimBlend (0 = blend, 1 = no blend); dispose is WebPMuxAnimDispose
 * (0 = none, 1 = background). Built by build.sh with the same flags as webpdump.c.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "src/webp/decode.h"
#include "src/webp/demux.h"
#include "src/dsp/cpu.h"

extern VP8CPUInfo VP8GetCPUInfo;

static uint64_t Fnv1a(const uint8_t* p, size_t n) {
  uint64_t h = 1469598103934665603ULL;
  size_t i;
  for (i = 0; i < n; ++i) {
    h ^= p[i];
    h *= 1099511628211ULL;
  }
  return h;
}

static uint8_t* ReadFile(const char* path, size_t* size) {
  FILE* f = fopen(path, "rb");
  long n;
  uint8_t* buf;
  if (f == NULL) return NULL;
  fseek(f, 0, SEEK_END);
  n = ftell(f);
  fseek(f, 0, SEEK_SET);
  buf = (uint8_t*)malloc(n > 0 ? (size_t)n : 1);
  if (buf != NULL && fread(buf, 1, (size_t)n, f) != (size_t)n) {
    free(buf);
    buf = NULL;
  }
  fclose(f);
  *size = (size_t)n;
  return buf;
}

static const char* Basename(const char* path) {
  const char* s = strrchr(path, '/');
  return s ? s + 1 : path;
}

static void PrintChunks(const char* base, const char* mode, WebPDemuxer* demux,
                        const char* fourcc) {
  WebPChunkIterator it;
  int ok = WebPDemuxGetChunk(demux, fourcc, 1, &it);
  while (ok) {
    printf("%s %s chunk=%s %d/%d size=%zu fnv=%016llx\n", base, mode, fourcc, it.chunk_num,
           it.num_chunks, it.chunk.size,
           (unsigned long long)Fnv1a(it.chunk.bytes, it.chunk.size));
    ok = WebPDemuxNextChunk(&it);
  }
  WebPDemuxReleaseChunkIterator(&it);
}

static void Report(const char* base, const char* mode, const WebPData* data, int partial) {
  WebPDemuxState state = WEBP_DEMUX_PARSE_ERROR;
  WebPDemuxer* demux = partial ? WebPDemuxPartial(data, &state)
                               : WebPDemuxInternal(data, 0, &state, WEBP_DEMUX_ABI_VERSION);
  if (demux == NULL) {
    printf("%s %s state=%d demux=0\n", base, mode, (int)state);
    return;
  }
  printf("%s %s state=%d demux=1 flags=%u canvas=%ux%u frames=%u loop=%u bgcolor=%08x\n", base,
         mode, (int)state, WebPDemuxGetI(demux, WEBP_FF_FORMAT_FLAGS),
         WebPDemuxGetI(demux, WEBP_FF_CANVAS_WIDTH), WebPDemuxGetI(demux, WEBP_FF_CANVAS_HEIGHT),
         WebPDemuxGetI(demux, WEBP_FF_FRAME_COUNT), WebPDemuxGetI(demux, WEBP_FF_LOOP_COUNT),
         WebPDemuxGetI(demux, WEBP_FF_BACKGROUND_COLOR));
  {
    WebPIterator iter;
    int ok = WebPDemuxGetFrame(demux, 1, &iter);
    while (ok) {
      printf("%s %s frame=%d x=%d y=%d w=%d h=%d dur=%d dispose=%d blend=%d complete=%d "
             "alpha=%d size=%zu fnv=%016llx\n",
             base, mode, iter.frame_num, iter.x_offset, iter.y_offset, iter.width, iter.height,
             iter.duration, (int)iter.dispose_method, (int)iter.blend_method, iter.complete,
             iter.has_alpha, iter.fragment.size,
             (unsigned long long)Fnv1a(iter.fragment.bytes, iter.fragment.size));
      ok = WebPDemuxNextFrame(&iter);
    }
    WebPDemuxReleaseIterator(&iter);
    ok = WebPDemuxGetFrame(demux, 0, &iter);
    printf("%s %s last frame=%d\n", base, mode, ok ? iter.frame_num : -1);
    WebPDemuxReleaseIterator(&iter);
  }
  PrintChunks(base, mode, demux, "ICCP");
  PrintChunks(base, mode, demux, "EXIF");
  PrintChunks(base, mode, demux, "XMP ");
  WebPDemuxDelete(demux);
}

int main(int argc, char** argv) {
  int i;
  /* Disable runtime CPU dispatch: every DSP entry point is the portable C one. */
  VP8GetCPUInfo = NULL;
  for (i = 1; i < argc; ++i) {
    size_t size = 0;
    uint8_t* data = ReadFile(argv[i], &size);
    WebPData webp_data;
    if (data == NULL) {
      printf("%s unreadable\n", Basename(argv[i]));
      continue;
    }
    webp_data.bytes = data;
    webp_data.size = size;
    Report(Basename(argv[i]), "full", &webp_data, 0);
    Report(Basename(argv[i]), "partial", &webp_data, 1);
    free(data);
  }
  return 0;
}
