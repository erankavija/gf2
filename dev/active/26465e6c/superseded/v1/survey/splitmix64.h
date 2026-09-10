#ifndef GF2_SURVEY_SPLITMIX64_H
#define GF2_SURVEY_SPLITMIX64_H

#include <stdint.h>

typedef struct { uint64_t state; } splitmix64_t;

static inline void splitmix64_init(splitmix64_t *g, uint64_t seed) { g->state = seed; }

static inline uint64_t splitmix64_next(splitmix64_t *g) {
    g->state += 0x9e3779b97f4a7c15ULL;
    uint64_t z = g->state;
    z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ULL;
    z = (z ^ (z >> 27)) * 0x94d049bb133111ebULL;
    return z ^ (z >> 31);
}

#endif
