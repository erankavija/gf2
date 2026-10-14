// Delays the creating thread after pthread_create returns, emulating its preemption.
#define _GNU_SOURCE
#include <dlfcn.h>
#include <pthread.h>
#include <stdlib.h>
#include <time.h>
int pthread_create(pthread_t *t, const pthread_attr_t *a, void *(*f)(void *), void *arg) {
    static int (*real)(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *);
    if (!real) real = dlsym(RTLD_NEXT, "pthread_create");
    int r = real(t, a, f, arg);
    const char *us = getenv("SPAWN_DELAY_US");
    if (us) {
        struct timespec s, n; clock_gettime(CLOCK_MONOTONIC, &s);
        long want = atol(us) * 1000;
        do clock_gettime(CLOCK_MONOTONIC, &n);
        while ((n.tv_sec - s.tv_sec) * 1000000000L + (n.tv_nsec - s.tv_nsec) < want);
    }
    return r;
}
