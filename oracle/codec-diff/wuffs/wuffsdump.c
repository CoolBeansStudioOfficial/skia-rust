/*
 * Differential dump of the Wuffs v0.3 GIF decoder, driven the way SkWuffsCodec drives it
 * (third_party/skia/src/codec/SkWuffsCodec.cpp: reset_and_decode_image_config, seekFrame,
 * resetDecoder, onGetFrameCountInternal, decodeFrameConfig, decodeFrame and fill_buffer). The Rust
 * port in crates/skia-rust-wuffs/tests/dump prints the same text; any difference is a
 * bug in the port.
 *
 * Per file, per variant and per read chunk size N (the most bytes one stream read returns):
 *   - the image config is decoded with short reads refilling a 4096-byte io buffer;
 *   - seekFrame(0) and decode_frame_config are called until end_of_data (the frame count);
 *   - for each of the first 8 frames, and for three pixel formats (BGRA_NONPREMUL,
 *     RGBA_NONPREMUL, BGR_565), the frame is re-seeked, its config decoded, and the frame
 *     decoded with decode_frame, refilling on short reads. The pixels are printed as an FNV-1a
 *     64 hash of the whole destination buffer, so partial output after a suspension is covered.
 *
 * Variants: the whole file; the first third; the first third with `closed` set at end of input
 * (so truncated input is reported as an error, not a suspension); the first two thirds, closed;
 * and the whole file with one byte flipped.
 *
 * Sweep: every truncation point of each file of at most 512 bytes, closed and not closed, at a
 * 4096-byte read size, reported as one line per cut (the image status, the first frame's status
 * and hash).
 *
 * Build (Linux, clang; the pinned Wuffs v0.3 release C file, wuffs-v0.3.c, at
 * google/wuffs-mirror-release-c@e3f919cc, release/c/wuffs-v0.3.c; the test
 * crates/skia-rust-wuffs/tests/differential.rs replays the committed expected output):
 *   clang -O2 -ffp-contract=off -I<dir of wuffs-v0.3.c> -o wuffsdump wuffsdump.c
 *
 * Usage: wuffsdump FILE...   (prints the dump for each file)
 */

#define WUFFS_IMPLEMENTATION
#include "wuffs-v0.3.c"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define IO_BUFFER_SIZE 4096
#define MAX_FRAMES 64
#define MAX_DECODED_FRAMES 8
#define SWEEP_MAX_LEN 512

typedef struct {
  const uint8_t* data;
  size_t len;
  size_t pos;
  size_t chunk;
} stream_t;

typedef struct {
  wuffs_gif__decoder* dec;
  wuffs_base__io_buffer io;
  uint8_t buf[IO_BUFFER_SIZE];
  stream_t s;
  int closed_on_eof;
  uint32_t width, height;
  uint64_t first_io;
  uint64_t frame_io[MAX_FRAMES];
  uint32_t frame_count;
  int suspended; /* SkWuffsCodec's fDecoderIsSuspended */
} session_t;

/* Set during the sweep: suppresses the per-step lines. */
static int quiet = 0;
#define OUT(...)           \
  do {                     \
    if (!quiet) {          \
      printf(__VA_ARGS__); \
    }                      \
  } while (0)

/* The status of frame 0 (BGRA, SRC) from the last run_decode, for the sweep. */
static const char* g_frame0_status = "skip";
static uint64_t g_frame0_hash = 0;

static size_t stream_read(stream_t* s, uint8_t* dst, size_t want) {
  size_t n = s->len - s->pos;
  if (n > want) {
    n = want;
  }
  if (n > s->chunk) {
    n = s->chunk;
  }
  memcpy(dst, s->data + s->pos, n);
  s->pos += n;
  return n;
}

/* Port of fill_buffer in SkWuffsCodec.cpp. */
static int fill(session_t* se) {
  wuffs_base__io_buffer* b = &se->io;
  /* Port of wuffs_base__io_buffer__compact. */
  if (b->meta.ri != 0) {
    b->meta.pos += b->meta.ri;
    size_t n = b->meta.wi - b->meta.ri;
    if (n != 0) {
      memmove(b->data.ptr, b->data.ptr + b->meta.ri, n);
    }
    b->meta.wi = n;
    b->meta.ri = 0;
  }
  size_t num_read = stream_read(&se->s, b->data.ptr + b->meta.wi, b->data.len - b->meta.wi);
  b->meta.wi += num_read;
  b->meta.closed = se->closed_on_eof && (num_read == 0);
  return num_read > 0;
}

/* After a failed fill, a client that has set `closed` calls the decoder once more, so the decoder
 * reports the truncation. Otherwise the input is incomplete and the caller stops. */
static int refill_or_closed(session_t* se) {
  return fill(se) || se->io.meta.closed;
}

/* Port of seek_buffer in SkWuffsCodec.cpp. */
static int seek(session_t* se, uint64_t pos) {
  wuffs_base__io_buffer* b = &se->io;
  if ((pos >= b->meta.pos) && (pos - b->meta.pos <= b->meta.wi)) {
    b->meta.ri = pos - b->meta.pos;
    return 1;
  }
  if (pos > se->s.len) {
    pos = se->s.len; /* SkMemoryStream::seek clamps to the length */
  }
  se->s.pos = (size_t)pos;
  b->meta.wi = 0;
  b->meta.ri = 0;
  b->meta.pos = pos;
  b->meta.closed = 0;
  return 1;
}

static uint64_t fnv1a(const uint8_t* p, size_t n) {
  uint64_t h = 1469598103934665603ull;
  for (size_t i = 0; i < n; i++) {
    h ^= p[i];
    h *= 1099511628211ull;
  }
  return h;
}

/* The status text: "ok" for success, the message (with its prefix) otherwise. */
static const char* status_text(wuffs_base__status st) {
  return st.repr ? st.repr : "ok";
}

/* Port of SkWuffsCodec::resetDecoder: rewinds the stream, clears the io buffer and decodes the
 * image config again, with no output. */
static int reset_decoder(session_t* se) {
  se->s.pos = 0;
  se->io.meta = wuffs_base__empty_io_buffer_meta();
  wuffs_base__status st = wuffs_gif__decoder__initialize(
      se->dec, sizeof__wuffs_gif__decoder(), WUFFS_VERSION, WUFFS_INITIALIZE__DEFAULT_OPTIONS);
  if (st.repr) {
    OUT("initialize %s\n", st.repr);
    return 0;
  }
  wuffs_gif__decoder__set_quirk_enabled(se->dec, WUFFS_GIF__QUIRK_IGNORE_TOO_MUCH_PIXEL_DATA,
                                        true);
  for (;;) {
    st = wuffs_gif__decoder__decode_image_config(se->dec, NULL, &se->io);
    if (st.repr == NULL) {
      break;
    }
    if (st.repr != wuffs_base__suspension__short_read) {
      OUT("reset %s\n", st.repr);
      return 0;
    }
    if (!refill_or_closed(se)) {
      OUT("reset incomplete\n");
      return 0;
    }
  }
  se->suspended = 0;
  return 1;
}

/* Port of SkWuffsCodec::seekFrame. */
static int seek_frame(session_t* se, uint32_t index) {
  if (se->suspended && !reset_decoder(se)) {
    return 0;
  }
  uint64_t pos = (index == 0) ? se->first_io : se->frame_io[index];
  if (!seek(se, pos)) {
    OUT("seek failed\n");
    return 0;
  }
  wuffs_base__status st = wuffs_gif__decoder__restart_frame(
      se->dec, index, wuffs_base__io_buffer__reader_io_position(&se->io));
  if (st.repr) {
    OUT("restart %u %s\n", index, st.repr);
    return 0;
  }
  return 1;
}

/* Port of SkWuffsCodec::decodeFrameConfig: refills on short reads. Returns the status text, or
 * "incomplete" when input ran out. */
static const char* decode_frame_config_loop(session_t* se, wuffs_base__frame_config* fc,
                                            wuffs_base__status* out) {
  wuffs_base__status st;
  for (;;) {
    st = wuffs_gif__decoder__decode_frame_config(se->dec, fc, &se->io);
    if (st.repr == wuffs_base__suspension__short_read && refill_or_closed(se)) {
      continue;
    }
    break;
  }
  *out = st;
  se->suspended = !wuffs_base__status__is_complete(&st);
  if (st.repr == wuffs_base__suspension__short_read) {
    return "incomplete";
  }
  return status_text(st);
}

static void print_frame_config(uint32_t index, wuffs_base__frame_config* fc) {
  wuffs_base__rect_ie_u32 r = fc->private_impl.bounds;
  OUT("frame %u ok bounds=%u,%u,%u,%u dur=%lld idx=%llu io=%llu disposal=%u opaque=%d "
      "overwrite=%d bg=%08x\n",
      index, r.min_incl_x, r.min_incl_y, r.max_excl_x, r.max_excl_y,
      (long long)fc->private_impl.duration, (unsigned long long)fc->private_impl.index,
      (unsigned long long)fc->private_impl.io_position, (unsigned)fc->private_impl.disposal,
      (int)fc->private_impl.opaque_within_bounds,
      (int)fc->private_impl.overwrite_instead_of_blend,
      (unsigned)fc->private_impl.background_color);
}

/* Returns "ok", "incomplete", "invalid", "seek" or the status message of the failure. */
static const char* run_config(session_t* se) {
  wuffs_base__image_config imgcfg = wuffs_base__null_image_config();
  wuffs_base__status st;
  for (;;) {
    st = wuffs_gif__decoder__decode_image_config(se->dec, &imgcfg, &se->io);
    if (st.repr == NULL) {
      break;
    }
    if (st.repr != wuffs_base__suspension__short_read) {
      OUT("image %s\n", st.repr);
      return st.repr;
    }
    if (!refill_or_closed(se)) {
      OUT("image incomplete\n");
      return "incomplete";
    }
  }
  se->width = imgcfg.pixcfg.private_impl.width;
  se->height = imgcfg.pixcfg.private_impl.height;
  se->first_io = wuffs_base__image_config__first_frame_io_position(&imgcfg);
  if (se->width == 0 || se->height == 0) {
    OUT("image invalid dimensions\n");
    return "invalid";
  }
  OUT("image ok width=%u height=%u first_io=%llu opaque=%d pixfmt=%08x\n", se->width,
      se->height, (unsigned long long)se->first_io,
      (int)wuffs_base__image_config__first_frame_is_opaque(&imgcfg),
      (unsigned)imgcfg.pixcfg.private_impl.pixfmt.repr);

  if (!seek_frame(se, 0)) {
    return "seek";
  }
  for (uint32_t k = 0; k < MAX_FRAMES; k++) {
    wuffs_base__frame_config fc = wuffs_base__null_frame_config();
    wuffs_base__status fst;
    const char* text = decode_frame_config_loop(se, &fc, &fst);
    if (fst.repr == NULL) {
      print_frame_config(k, &fc);
      se->frame_io[k] = fc.private_impl.io_position;
      se->frame_count = k + 1;
      continue;
    }
    if (fst.repr == wuffs_base__note__end_of_data) {
      OUT("frame %u end\n", k);
    } else {
      OUT("frame %u %s\n", k, text);
    }
    break;
  }
  OUT("frames n=%u loops=%u\n", se->frame_count,
      (unsigned)wuffs_gif__decoder__num_animation_loops(se->dec));
  return "ok";
}

static void run_decode(session_t* se) {
  static const uint32_t fmts[3] = {0x81008888, 0xA1008888, 0x80000565};
  static const char* fmt_names[3] = {"bgra", "rgba", "565"};
  uint32_t n = se->frame_count < MAX_DECODED_FRAMES ? se->frame_count : MAX_DECODED_FRAMES;
  g_frame0_status = "skip";
  g_frame0_hash = 0;
  for (uint32_t i = 0; i < n; i++) {
    for (int c = 0; c < 3; c++) {
      wuffs_base__pixel_blend blend = (i == 0) ? WUFFS_BASE__PIXEL_BLEND__SRC
                                               : WUFFS_BASE__PIXEL_BLEND__SRC_OVER;
      const char* blend_name = (i == 0) ? "src" : "srcover";
      uint32_t bpp = (c == 2) ? 2 : 4;
      if (!seek_frame(se, i)) {
        continue;
      }
      wuffs_base__frame_config fc = wuffs_base__null_frame_config();
      wuffs_base__status fst;
      const char* text = decode_frame_config_loop(se, &fc, &fst);
      OUT("decode %u %s %s frameconfig=%s\n", i, fmt_names[c], blend_name, text);
      if (fst.repr != NULL) {
        continue;
      }
      size_t stride = (size_t)se->width * bpp;
      size_t buf_len = stride * se->height;
      uint8_t* pixels = calloc(buf_len, 1);
      wuffs_base__pixel_buffer pb = wuffs_base__null_pixel_buffer();
      wuffs_base__pixel_config pc = wuffs_base__null_pixel_config();
      wuffs_base__pixel_config__set(&pc, fmts[c], WUFFS_BASE__PIXEL_SUBSAMPLING__NONE,
                                    se->width, se->height);
      wuffs_base__table_u8 tab;
      tab.ptr = pixels;
      tab.width = (size_t)se->width * bpp;
      tab.height = se->height;
      tab.stride = stride;
      wuffs_base__status pst = wuffs_base__pixel_buffer__set_from_table(&pb, &pc, tab);
      if (pst.repr) {
        OUT("set_from_table %s\n", pst.repr);
        free(pixels);
        continue;
      }
      wuffs_base__status st;
      for (;;) {
        st = wuffs_gif__decoder__decode_frame(se->dec, &pb, &se->io, blend,
                                              wuffs_base__make_slice_u8(NULL, 0), NULL);
        if (st.repr == wuffs_base__suspension__short_read && refill_or_closed(se)) {
          continue;
        }
        break;
      }
      se->suspended = !wuffs_base__status__is_complete(&st);
      const char* status = st.repr == wuffs_base__suspension__short_read ? "incomplete"
                                                                         : status_text(st);
      uint64_t hash = fnv1a(pixels, buf_len);
      if (i == 0 && c == 0) {
        g_frame0_status = status;
        g_frame0_hash = hash;
      }
      wuffs_base__rect_ie_u32 d = wuffs_gif__decoder__frame_dirty_rect(se->dec);
      OUT("decode %u %s %s status=%s hash=%016llx dirty=%u,%u,%u,%u nconf=%llu nframes=%llu\n",
          i, fmt_names[c], blend_name, status, (unsigned long long)hash, d.min_incl_x,
          d.min_incl_y, d.max_excl_x, d.max_excl_y,
          (unsigned long long)wuffs_gif__decoder__num_decoded_frame_configs(se->dec),
          (unsigned long long)wuffs_gif__decoder__num_decoded_frames(se->dec));
      free(pixels);
    }
  }
}

static session_t* session_new(const uint8_t* data, size_t len, size_t chunk, int closed_on_eof) {
  session_t* se = calloc(1, sizeof(session_t));
  se->dec = calloc(1, sizeof__wuffs_gif__decoder());
  wuffs_gif__decoder__initialize(se->dec, sizeof__wuffs_gif__decoder(), WUFFS_VERSION,
                                 WUFFS_INITIALIZE__DEFAULT_OPTIONS);
  /* SkWuffsCodec sets this quirk on every decoder
   * (https://bugs.chromium.org/p/skia/issues/detail?id=12055). */
  wuffs_gif__decoder__set_quirk_enabled(se->dec, WUFFS_GIF__QUIRK_IGNORE_TOO_MUCH_PIXEL_DATA,
                                        true);
  se->io = wuffs_base__make_io_buffer(wuffs_base__make_slice_u8(se->buf, IO_BUFFER_SIZE),
                                      wuffs_base__empty_io_buffer_meta());
  se->s.data = data;
  se->s.len = len;
  se->s.pos = 0;
  se->s.chunk = chunk;
  se->closed_on_eof = closed_on_eof;
  se->suspended = 0;
  return se;
}

static void session_free(session_t* se) {
  free(se->dec);
  free(se);
}

static void run_variant(const char* name, const uint8_t* data, size_t len, size_t chunk,
                        const char* variant, int closed_on_eof) {
  OUT("== %s chunk=%zu variant=%s len=%zu\n", name, chunk, variant, len);
  session_t* se = session_new(data, len, chunk, closed_on_eof);
  run_config(se);
  run_decode(se);
  session_free(se);
}

/* One line per truncation point, for the sweep. */
static void sweep_cut(const uint8_t* data, size_t cut, int closed_on_eof) {
  quiet = 1;
  session_t* se = session_new(data, cut, 4096, closed_on_eof);
  const char* cfg = run_config(se);
  g_frame0_status = "skip";
  g_frame0_hash = 0;
  if (strcmp(cfg, "ok") == 0 && se->frame_count > 0) {
    run_decode(se);
  }
  quiet = 0;
  printf("cut %zu closed=%d config=%s frame0=%s hash=%016llx\n", cut, closed_on_eof, cfg,
         g_frame0_status, (unsigned long long)g_frame0_hash);
  session_free(se);
}

static uint8_t* read_file(const char* path, size_t* len) {
  FILE* f = fopen(path, "rb");
  if (!f) {
    return NULL;
  }
  fseek(f, 0, SEEK_END);
  long n = ftell(f);
  fseek(f, 0, SEEK_SET);
  uint8_t* p = malloc(n > 0 ? (size_t)n : 1);
  size_t got = fread(p, 1, (size_t)n, f);
  fclose(f);
  *len = got;
  return p;
}

int main(int argc, char** argv) {
  for (int a = 1; a < argc; a++) {
    size_t len = 0;
    uint8_t* data = read_file(argv[a], &len);
    if (!data) {
      printf("== %s unreadable\n", argv[a]);
      continue;
    }
    const char* base = strrchr(argv[a], '/');
    base = base ? base + 1 : argv[a];
    /* One byte at a time for small files (every suspension point), then 7 bytes, then a full
     * 4096-byte read. */
    size_t chunks[3] = {1, 7, 4096};
    int nchunks = len <= 8192 ? 3 : (len <= 102400 ? 2 : 1);
    for (int c = 0; c < nchunks; c++) {
      size_t chunk = chunks[c];
      run_variant(base, data, len, chunk, "full", 0);
      if (chunk == 4096) {
        run_variant(base, data, len / 3, chunk, "trunc1of3", 0);
        run_variant(base, data, len / 3, chunk, "trunc1of3closed", 1);
        run_variant(base, data, (2 * len) / 3, chunk, "trunc2of3closed", 1);
        if (len > 0) {
          uint8_t* corrupt = malloc(len);
          memcpy(corrupt, data, len);
          corrupt[len / 2] ^= 0x5A;
          run_variant(base, corrupt, len, chunk, "corrupt", 0);
          free(corrupt);
        }
      }
    }
    if (len <= SWEEP_MAX_LEN) {
      printf("== %s sweep len=%zu\n", base, len);
      for (size_t cut = 0; cut <= len; cut++) {
        sweep_cut(data, cut, 0);
        sweep_cut(data, cut, 1);
      }
    }
    free(data);
  }
  return 0;
}
