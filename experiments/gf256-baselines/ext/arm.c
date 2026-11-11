/* ISA-L and M4RIE arms of the GF(2^8) comparison on the cells, inputs,
 * repetitions and checksums of src/main.rs, one line per cell and
 * configuration. Modulus 0x11D throughout. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <isa-l/erasure_code.h>
#include <m4rie/m4rie.h>

#define BATCHES 5

static uint64_t state;
static uint8_t next_byte(void) {
  state += 0x9e3779b97f4a7c15ull;
  uint64_t z = state;
  z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ull;
  z = (z ^ (z >> 27)) * 0x94d049bb133111ebull;
  return (uint8_t)((z ^ (z >> 31)) >> 56);
}

static uint64_t checksum(const uint8_t *bytes, size_t len) {
  uint64_t hash = 0xcbf29ce484222325ull;
  for (size_t i = 0; i < len; i++) hash = (hash ^ bytes[i]) * 0x100000001b3ull;
  return hash;
}

static double now(void) {
  struct timespec t;
  clock_gettime(CLOCK_MONOTONIC, &t);
  return t.tv_sec + t.tv_nsec * 1e-9;
}

/* Best batch time of BATCHES timed batches after one untimed. */
#define BEST(result, body)                                         \
  do {                                                             \
    body;                                                          \
    result = 1e30;                                                 \
    for (int batch = 0; batch < BATCHES; batch++) {                \
      double start = now();                                        \
      body;                                                        \
      double elapsed = now() - start;                              \
      if (elapsed < result) result = elapsed;                      \
    }                                                              \
  } while (0)

static void axpy(size_t len) {
  state = len;
  uint8_t c = next_byte();
  uint8_t *x = malloc(len), *y = malloc(len), tables[32];
  for (size_t i = 0; i < len; i++) x[i] = next_byte();
  for (size_t i = 0; i < len; i++) y[i] = next_byte();
  size_t reps = (((size_t)1 << 28) / len) ? ((size_t)1 << 28) / len : 1;
  gf_vect_mul_init(c, tables);
  /* Two equal updates cancel, so the digest is of one update of the drawn y,
   * taken before the timed repetitions. */
  uint8_t *once = malloc(len);
  memcpy(once, y, len);
  gf_vect_mad((int)len, 1, 0, tables, x, once);
  uint64_t digest = checksum(once, len);
  double seconds;
  BEST(seconds, for (size_t r = 0; r < reps; r++) gf_vect_mad((int)len, 1, 0, tables, x, y));
  printf("axpy\t%zu\tisa-l gf_vect_mad\t%.2f GB/s\t%016llx\n", len,
         (double)len * reps / seconds / 1e9, (unsigned long long)digest);
  free(once);
  free(x);
  free(y);
}

static void read_out(const mzed_t *C, uint8_t *out, int n) {
  for (int i = 0; i < n; i++)
    for (int j = 0; j < n; j++) out[i * n + j] = (uint8_t)mzed_read_elem(C, i, j);
}

static void report(const char *name, int n, int reps, double seconds, const uint8_t *out) {
  seconds /= reps;
  printf("gemm\t%d\tm4rie %s\t%.3f ms %.2f Gmul-add/s\t%016llx\n", n, name, seconds * 1e3,
         (double)n * n * n / seconds / 1e9, (unsigned long long)checksum(out, (size_t)n * n));
}

static void gemm(gf2e *ff, int n) {
  state = (uint64_t)n ^ 0x6e6d;
  mzed_t *A = mzed_init(ff, n, n), *B = mzed_init(ff, n, n), *C = mzed_init(ff, n, n);
  for (int i = 0; i < n; i++)
    for (int j = 0; j < n; j++) mzed_write_elem(A, i, j, next_byte());
  for (int i = 0; i < n; i++)
    for (int j = 0; j < n; j++) mzed_write_elem(B, i, j, next_byte());
  int reps = n == 64 ? 4096 : n == 256 ? 64 : 2;
  uint8_t *out = malloc((size_t)n * n);
  double seconds;

  BEST(seconds, for (int r = 0; r < reps; r++) mzed_mul(C, A, B));
  read_out(C, out, n);
  report("mzed_mul", n, reps, seconds, out);

  BEST(seconds, for (int r = 0; r < reps; r++) mzed_mul_newton_john(C, A, B));
  read_out(C, out, n);
  report("mzed_mul_newton_john", n, reps, seconds, out);

  /* Karatsuba runs on the bitsliced form, converted once outside the timing. */
  mzd_slice_t *As = mzed_slice(NULL, A), *Bs = mzed_slice(NULL, B), *Cs = mzd_slice_init(ff, n, n);
  BEST(seconds, for (int r = 0; r < reps; r++) mzd_slice_mul_karatsuba(Cs, As, Bs));
  mzed_cling(C, Cs);
  read_out(C, out, n);
  report("mzd_slice_mul_karatsuba", n, reps, seconds, out);

  mzd_slice_free(As);
  mzd_slice_free(Bs);
  mzd_slice_free(Cs);
  mzed_free(A);
  mzed_free(B);
  mzed_free(C);
  free(out);
}

int main(void) {
  const size_t lengths[] = {4 << 10, 64 << 10, 1 << 20, 16 << 20};
  for (int i = 0; i < 4; i++) axpy(lengths[i]);
  gf2e *ff = gf2e_init(0x11D);
  const int sizes[] = {64, 256, 1024};
  for (int i = 0; i < 3; i++) gemm(ff, sizes[i]);
  gf2e_free(ff);
  return 0;
}
