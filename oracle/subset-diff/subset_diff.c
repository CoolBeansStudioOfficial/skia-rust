/* Calls hb-subset exactly as Skia's src/pdf/SkPDFSubsetFont.cpp does (stream_to_face + subset_harfbuzz
 * + make_subset + to_data) for every line of a corpus file and writes the result blobs.
 *
 *   subset_diff <font-dir> <corpus.txt> <out-dir>
 *
 * corpus.txt lines: `<font file> <ttc index> <set name> <gid>,<gid>,...` (see gen_corpus.py). For
 * each line the driver writes <out-dir>/<font file>.<index>.<set name>.bin (nothing on failure) and
 * prints `<font file> <index> <set name> <length|FAIL>`. */
#include <hb.h>
#include <hb-subset.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static char* read_file(const char* path, unsigned* len) {
    FILE* f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    fseek(f, 0, SEEK_SET);
    char* p = (char*)malloc(n ? n : 1);
    if (fread(p, 1, n, f) != (size_t)n) { fclose(f); free(p); return NULL; }
    fclose(f);
    *len = (unsigned)n;
    return p;
}

int main(int argc, char** argv) {
    if (argc != 4) { fprintf(stderr, "usage: subset_diff <font-dir> <corpus> <out-dir>\n"); return 2; }
    FILE* corpus = fopen(argv[2], "r");
    if (!corpus) { perror("corpus"); return 2; }
    static char line[1 << 20];
    while (fgets(line, sizeof line, corpus)) {
        char font[256], set[256];
        unsigned index;
        int consumed = 0;
        if (line[0] == '#' || line[0] == '\n') continue;
        if (sscanf(line, "%255s %u %255s %n", font, &index, set, &consumed) < 3) continue;
        char path[1024];
        snprintf(path, sizeof path, "%s/%s", argv[1], font);
        unsigned len;
        char* data = read_file(path, &len);
        if (!data) { printf("%s %u %s FAIL\n", font, index, set); continue; }
        /* stream_to_blob + stream_to_face */
        hb_blob_t* blob = hb_blob_create(data, len, HB_MEMORY_MODE_READONLY, data, free);
        hb_blob_make_immutable(blob);
        hb_face_t* face = NULL;
        unsigned num_faces = hb_face_count(blob);
        if (0 < num_faces && index < num_faces) {
            face = hb_face_create(blob, index);
            if (face && hb_face_get_glyph_count(face) == 0) { hb_face_destroy(face); face = NULL; }
        }
        hb_subset_input_t* input = hb_subset_input_create_or_fail();
        int has_zero = 0;
        hb_blob_t* result = NULL;
        if (face && input) {
            hb_set_t* glyphs = hb_subset_input_glyph_set(input);
            for (const char* p = line + consumed; *p && *p != '\n';) {
                char* end;
                unsigned long g = strtoul(p, &end, 10);
                if (end == p) break;
                hb_set_add(glyphs, (hb_codepoint_t)g);
                if (g == 0) has_zero = 1;
                p = *end == ',' ? end + 1 : end;
            }
            /* make_subset */
            unsigned flags = HB_SUBSET_FLAGS_RETAIN_GIDS;
            if (has_zero) flags |= HB_SUBSET_FLAGS_NOTDEF_OUTLINE;
            hb_subset_input_set_flags(input, flags);
            hb_face_t* subset = hb_subset_or_fail(face, input);
            if (subset) {
                hb_blob_t* b = hb_face_reference_blob(subset);
                unsigned n = 0;
                const char* d = hb_blob_get_data(b, &n);
                if (d && n) { result = b; } else { hb_blob_destroy(b); }
                if (result) {
                    char out[1024];
                    snprintf(out, sizeof out, "%s/%s.%u.%s.bin", argv[3], font, index, set);
                    FILE* o = fopen(out, "wb");
                    fwrite(d, 1, n, o);
                    fclose(o);
                    printf("%s %u %s %u\n", font, index, set, n);
                }
                hb_face_destroy(subset);
            }
        }
        if (!result) printf("%s %u %s FAIL\n", font, index, set);
        else hb_blob_destroy(result);
        if (input) hb_subset_input_destroy(input);
        if (face) hb_face_destroy(face);
        hb_blob_destroy(blob);
    }
    return 0;
}
