/* SPDX-License-Identifier: GPL-2.0-only */
#ifdef NDEBUG
#undef NDEBUG
#endif
#include <assert.h>
#include <inttypes.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#ifdef _WIN32
#include <windows.h>
#endif

typedef uint32_t u32;
#ifndef __always_inline
#define __always_inline inline __attribute__((always_inline))
#endif
#define READ_ONCE(x) (*(volatile __typeof__(x) *)&(x))
#define likely(x) (x)
struct hybridmount_rule { size_t child_len; char name[24]; };
struct hybridmount_child_array { int count, capacity; u32 hashes[]; };
#define hm_get_child_rules(a) ((struct hybridmount_rule **)((a)->hashes + (a)->capacity))
#define hm_get_child_name(r) ((r)->name)
#include "production.h"

enum { QUERY_COUNT = 65536, REPEATS = 32, SAMPLES = 5 };
struct query { u32 hash; size_t len; char name[24]; };
struct directory {
    struct hybridmount_child_array *arr;
    struct hybridmount_rule *rules;
    uint64_t fixed, adaptive[64];
    unsigned words;
};
static volatile uintptr_t sink;
static uint64_t rng = UINT64_C(0x79be1258d4c903a7);
static u32 next_hash(void)
{
    rng ^= rng >> 12; rng ^= rng << 25; rng ^= rng >> 27;
    return (u32)((rng * UINT64_C(2685821657736338717)) >> 32);
}
static double seconds(void)
{
#ifdef _WIN32
    LARGE_INTEGER t, f;
    QueryPerformanceCounter(&t); QueryPerformanceFrequency(&f);
    return (double)t.QuadPart / (double)f.QuadPart;
#else
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) abort();
    return (double)t.tv_sec + (double)t.tv_nsec * 1e-9;
#endif
}
static bool gate(const struct directory *d, u32 hash, bool adaptive)
{
    return adaptive ? !!(d->adaptive[(hash >> 6) & (d->words - 1)] & (UINT64_C(1) << (hash & 63)))
                    : !!(d->fixed & (UINT64_C(1) << (hash & 63)));
}
static struct hybridmount_rule *lookup(struct directory *d, const struct query *q, int mode)
{
    if (mode == 2 && d->arr->count <= 2) {
        for (int i = 0; i < d->arr->count; i++) {
            struct hybridmount_rule *r = hm_get_child_rules(d->arr)[i];
            if (d->arr->hashes[i] == q->hash && r->child_len == q->len && !memcmp(r->name, q->name, q->len)) return r;
        }
        return NULL;
    }
    if (!gate(d, q->hash, mode != 0)) return NULL;
    if (d->arr->count == 1) {
        struct hybridmount_rule *r = &d->rules[0];
        return r->child_len == q->len && !memcmp(r->name, q->name, q->len) ? r : NULL;
    }
    return hybridmount_bsearch_child(d->arr, q->name, q->len, q->hash, NULL);
}
static void init(struct directory *d, int count)
{
    memset(d, 0, sizeof(*d));
    d->words = count <= 16 ? 4 : count <= 64 ? 16 : 64;
    int cap = 4;
    while (cap < count) cap *= 2;
    d->arr = calloc(1, sizeof(*d->arr) + cap * (sizeof(u32) + sizeof(void *)));
    d->rules = calloc((size_t)count, sizeof(*d->rules));
    assert(d->arr && d->rules);
    d->arr->count = count; d->arr->capacity = cap;
    for (int i = 0; i < count; i++) {
        u32 hash = next_hash();
        int pos = i;
        while (pos && d->arr->hashes[pos - 1] > hash) {
            d->arr->hashes[pos] = d->arr->hashes[pos - 1];
            hm_get_child_rules(d->arr)[pos] = hm_get_child_rules(d->arr)[pos - 1];
            pos--;
        }
        d->rules[i].child_len = (size_t)snprintf(d->rules[i].name, sizeof(d->rules[i].name), "child-%08x", hash);
        d->arr->hashes[pos] = hash;
        hm_get_child_rules(d->arr)[pos] = &d->rules[i];
        d->fixed |= UINT64_C(1) << (hash & 63);
        d->adaptive[(hash >> 6) & (d->words - 1)] |= UINT64_C(1) << (hash & 63);
    }
}
static int compare_double(const void *a, const void *b)
{
    double x = *(const double *)a, y = *(const double *)b;
    return (x > y) - (x < y);
}
static void collision_selftest(void)
{
    struct directory d;
    init(&d, 2);
    d.arr->hashes[0] = d.arr->hashes[1] = 7;
    struct hybridmount_rule **rules = hm_get_child_rules(d.arr);
    for (int i = 0; i < 2; i++) {
        int index = -1;
        assert(hybridmount_bsearch_child(d.arr, rules[i]->name, rules[i]->child_len, 7, &index) == rules[i]);
        assert(index == i);
    }
    assert(!hybridmount_bsearch_child(d.arr, "missing", 7, 7, NULL));
    assert(!hybridmount_bsearch_child(d.arr, rules[0]->name, rules[0]->child_len, 8, NULL));
    free(d.rules); free(d.arr);
}
int main(void)
{
    static const int counts[] = {1, 2, 16, 64, 256};
    static const char *modes[] = {"fixed64", "adaptive", "adaptive_direct2"};
    struct query *queries = calloc(QUERY_COUNT, sizeof(*queries));
    assert(queries);
    collision_selftest();
    rng = UINT64_C(0x79be1258d4c903a7);
    puts("# medians of 5 hot-cache samples; each sample 65536 queries x 32; seed 0x79be1258d4c903a7");
    puts("# filter_bytes are logical payload, exclude allocation/alignment/metadata and existing dir-node mask");
    puts("children,workload,mode,filter_bytes,gate_pass_percent,median_ns_per_query");
    for (unsigned c = 0; c < sizeof(counts) / sizeof(counts[0]); c++) {
        struct directory d;
        init(&d, counts[c]);
        for (int hits = 0; hits <= 1; hits++) {
            for (unsigned i = 0; i < QUERY_COUNT; i++) {
                struct query *q = &queries[i];
                if (hits) {
                    int pos = (int)(next_hash() % (u32)d.arr->count);
                    struct hybridmount_rule *r = hm_get_child_rules(d.arr)[pos];
                    q->hash = d.arr->hashes[pos]; q->len = r->child_len;
                    memcpy(q->name, r->name, r->child_len + 1);
                } else {
                    q->hash = next_hash();
                    q->len = (size_t)snprintf(q->name, sizeof(q->name), "absnt-%08x", q->hash);
                }
            }
            for (unsigned i = 0; i < QUERY_COUNT; i++) {
                struct hybridmount_rule *expected = NULL;
                for (int j = 0; j < d.arr->count; j++) {
                    struct hybridmount_rule *r = &d.rules[j];
                    if (r->child_len == queries[i].len && !memcmp(r->name, queries[i].name, queries[i].len)) expected = r;
                }
                for (int mode = 0; mode < 3; mode++) assert(lookup(&d, &queries[i], mode) == expected);
            }
            double times[3][SAMPLES];
            for (int sample = 0; sample < SAMPLES; sample++) {
                for (int order = 0; order < 3; order++) {
                    int mode = (sample + order) % 3;
                    uintptr_t sum = 0;
                    double start = seconds();
                    for (int rep = 0; rep < REPEATS; rep++)
                        for (unsigned i = 0; i < QUERY_COUNT; i++)
                            sum += (uintptr_t)lookup(&d, &queries[i], mode);
                    times[mode][sample] = (seconds() - start) * 1e9 / (QUERY_COUNT * REPEATS);
                    sink = sum;
                }
            }
            for (int mode = 0; mode < 3; mode++) {
                unsigned passed = 0;
                for (unsigned i = 0; i < QUERY_COUNT; i++)
                    passed += mode == 2 && d.arr->count <= 2 ? 1 : gate(&d, queries[i].hash, mode != 0);
                qsort(times[mode], SAMPLES, sizeof(double), compare_double);
                printf("%d,%s,%s,%u,%.4f,%.3f\n", d.arr->count, hits ? "positive" : "negative", modes[mode],
                       mode == 0 ? 8 : mode == 2 && d.arr->count <= 2 ? 0 : d.words * 8,
                       passed * 100.0 / QUERY_COUNT, times[mode][SAMPLES / 2]);
            }
        }
        free(d.rules); free(d.arr);
    }
    free(queries);
    return 0;
}
