#ifndef GF2_JSON_MIN_H
#define GF2_JSON_MIN_H

#include <stddef.h>
#include <stdint.h>

typedef enum {
    JSON_NULL,
    JSON_BOOL,
    JSON_NUMBER,
    JSON_STRING,
    JSON_ARRAY,
    JSON_OBJECT
} json_type;

typedef struct json_value json_value;

typedef struct {
    char *key;
    json_value *value;
} json_member;

struct json_value {
    json_type type;
    union {
        int boolean;
        char *number;
        char *string;
        struct {
            json_value **items;
            size_t length;
        } array;
        struct {
            json_member *members;
            size_t length;
        } object;
    } as;
};

int json_parse(const char *input, size_t length, json_value **out, char **error);
void json_free(json_value *value);
const json_value *json_object_get(const json_value *object, const char *key);
int json_get_string(const json_value *value, const char **out);
int json_get_u64(const json_value *value, uint64_t *out);

#endif
