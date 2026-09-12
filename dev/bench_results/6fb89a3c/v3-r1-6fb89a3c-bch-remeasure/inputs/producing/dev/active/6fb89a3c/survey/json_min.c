#define _POSIX_C_SOURCE 200809L
#include "json_min.h"

#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    const char *start;
    const char *at;
    const char *end;
    char message[160];
} parser;

static void set_error(parser *p, const char *message)
{
    if (p->message[0] == '\0')
        (void)snprintf(p->message, sizeof(p->message), "%s at byte %zu", message,
        (size_t)(p->at - p->start));
}

static void skip_space(parser *p)
{
    while (p->at < p->end && isspace((unsigned char)*p->at))
        p->at++;
}

static json_value *new_value(json_type type)
{
    json_value *value = (json_value *)calloc(1, sizeof(*value));
    if (value != NULL)
        value->type = type;
    return value;
}

static int append_byte(char **buffer, size_t *length, size_t *capacity, unsigned char byte)
{
    if (*length == *capacity) {
        size_t next = *capacity == 0 ? 32 : *capacity * 2;
        char *grown = (char *)realloc(*buffer, next);
        if (grown == NULL)
            return 0;
        *buffer = grown;
        *capacity = next;
    }
    (*buffer)[(*length)++] = (char)byte;
    return 1;
}

static int hex_digit(char c)
{
    if (c >= '0' && c <= '9')
        return c - '0';
    if (c >= 'a' && c <= 'f')
        return c - 'a' + 10;
    if (c >= 'A' && c <= 'F')
        return c - 'A' + 10;
    return -1;
}

static int append_utf8(char **buffer, size_t *length, size_t *capacity, unsigned value)
{
    if (value <= 0x7f)
        return append_byte(buffer, length, capacity, (unsigned char)value);
    if (value <= 0x7ff)
        return append_byte(buffer, length, capacity, (unsigned char)(0xc0 | (value >> 6)))
            && append_byte(buffer, length, capacity, (unsigned char)(0x80 | (value & 0x3f)));
    if (value >= 0xd800 && value <= 0xdfff)
        return 0;
    if (value <= 0xffff)
        return append_byte(buffer, length, capacity, (unsigned char)(0xe0 | (value >> 12)))
            && append_byte(buffer, length, capacity, (unsigned char)(0x80 | ((value >> 6) & 0x3f)))
            && append_byte(buffer, length, capacity, (unsigned char)(0x80 | (value & 0x3f)));
    if (value <= 0x10ffff)
        return append_byte(buffer, length, capacity, (unsigned char)(0xf0 | (value >> 18)))
            && append_byte(buffer, length, capacity, (unsigned char)(0x80 | ((value >> 12) & 0x3f)))
            && append_byte(buffer, length, capacity, (unsigned char)(0x80 | ((value >> 6) & 0x3f)))
            && append_byte(buffer, length, capacity, (unsigned char)(0x80 | (value & 0x3f)));
    return 0;
}

static char *parse_string(parser *p)
{
    char *buffer = NULL;
    size_t length = 0, capacity = 0;
    if (p->at >= p->end || *p->at++ != '"') {
        set_error(p, "expected string");
        return NULL;
    }
    while (p->at < p->end) {
        unsigned char c = (unsigned char)*p->at++;
        if (c == '"') {
            if (!append_byte(&buffer, &length, &capacity, 0))
                goto oom;
            return buffer;
        }
        if (c < 0x20)
            goto bad;
        if (c != '\\') {
            if (!append_byte(&buffer, &length, &capacity, c))
                goto oom;
            continue;
        }
        if (p->at >= p->end)
            goto bad;
        c = (unsigned char)*p->at++;
        switch (c) {
        case '"': case '\\': case '/':
            if (!append_byte(&buffer, &length, &capacity, c)) goto oom;
            break;
        case 'b': if (!append_byte(&buffer, &length, &capacity, '\b')) goto oom; break;
        case 'f': if (!append_byte(&buffer, &length, &capacity, '\f')) goto oom; break;
        case 'n': if (!append_byte(&buffer, &length, &capacity, '\n')) goto oom; break;
        case 'r': if (!append_byte(&buffer, &length, &capacity, '\r')) goto oom; break;
        case 't': if (!append_byte(&buffer, &length, &capacity, '\t')) goto oom; break;
        case 'u': {
            unsigned value = 0;
            int i;
            for (i = 0; i < 4; i++) {
                if (p->at >= p->end || hex_digit(*p->at) < 0) goto bad;
                value = (value << 4) | (unsigned)hex_digit(*p->at++);
            }
            if (!append_utf8(&buffer, &length, &capacity, value)) goto bad;
            break;
        }
        default:
            goto bad;
        }
    }
bad:
    free(buffer);
    set_error(p, "invalid string escape or unterminated string");
    return NULL;
oom:
    free(buffer);
    set_error(p, "out of memory");
    return NULL;
}

static json_value *parse_value(parser *p);

static json_value *parse_array(parser *p)
{
    json_value *value = new_value(JSON_ARRAY);
    if (value == NULL) { set_error(p, "out of memory"); return NULL; }
    p->at++;
    skip_space(p);
    if (p->at < p->end && *p->at == ']') { p->at++; return value; }
    while (p->at < p->end) {
        json_value *item = parse_value(p);
        if (item == NULL) { json_free(value); return NULL; }
        json_value **grown = (json_value **)realloc(value->as.array.items,
                                                     (value->as.array.length + 1) * sizeof(*grown));
        if (grown == NULL) { json_free(item); json_free(value); set_error(p, "out of memory"); return NULL; }
        value->as.array.items = grown;
        value->as.array.items[value->as.array.length++] = item;
        skip_space(p);
        if (p->at < p->end && *p->at == ']') { p->at++; return value; }
        if (p->at >= p->end || *p->at++ != ',') break;
        skip_space(p);
    }
    json_free(value);
    set_error(p, "invalid array");
    return NULL;
}

static json_value *parse_object(parser *p)
{
    json_value *value = new_value(JSON_OBJECT);
    if (value == NULL) { set_error(p, "out of memory"); return NULL; }
    p->at++;
    skip_space(p);
    if (p->at < p->end && *p->at == '}') { p->at++; return value; }
    while (p->at < p->end) {
        char *key;
        json_value *member_value;
        json_member *grown;
        if (p->at >= p->end || *p->at != '"') break;
        key = parse_string(p);
        if (key == NULL) { json_free(value); return NULL; }
        skip_space(p);
        if (p->at >= p->end || *p->at++ != ':') { free(key); json_free(value); set_error(p, "expected colon"); return NULL; }
        skip_space(p);
        member_value = parse_value(p);
        if (member_value == NULL) { free(key); json_free(value); return NULL; }
        grown = (json_member *)realloc(value->as.object.members,
                                       (value->as.object.length + 1) * sizeof(*grown));
        if (grown == NULL) { free(key); json_free(member_value); json_free(value); set_error(p, "out of memory"); return NULL; }
        value->as.object.members = grown;
        value->as.object.members[value->as.object.length].key = key;
        value->as.object.members[value->as.object.length++].value = member_value;
        skip_space(p);
        if (p->at < p->end && *p->at == '}') { p->at++; return value; }
        if (p->at >= p->end || *p->at++ != ',') break;
        skip_space(p);
    }
    json_free(value);
    set_error(p, "invalid object");
    return NULL;
}

static json_value *parse_number(parser *p)
{
    const char *start = p->at;
    char *end;
    json_value *value;
    if (*p->at == '-') p->at++;
    if (p->at >= p->end || !isdigit((unsigned char)*p->at)) goto bad;
    if (*p->at == '0') p->at++;
    else while (p->at < p->end && isdigit((unsigned char)*p->at)) p->at++;
    if (p->at < p->end && *p->at == '.') {
        p->at++;
        if (p->at >= p->end || !isdigit((unsigned char)*p->at)) goto bad;
        while (p->at < p->end && isdigit((unsigned char)*p->at)) p->at++;
    }
    if (p->at < p->end && (*p->at == 'e' || *p->at == 'E')) {
        p->at++; if (p->at < p->end && (*p->at == '+' || *p->at == '-')) p->at++;
        if (p->at >= p->end || !isdigit((unsigned char)*p->at)) goto bad;
        while (p->at < p->end && isdigit((unsigned char)*p->at)) p->at++;
    }
    value = new_value(JSON_NUMBER);
    if (value == NULL) { set_error(p, "out of memory"); return NULL; }
    end = (char *)malloc((size_t)(p->at - start) + 1);
    if (end == NULL) { json_free(value); set_error(p, "out of memory"); return NULL; }
    memcpy(end, start, (size_t)(p->at - start)); end[p->at - start] = '\0';
    value->as.number = end;
    return value;
bad:
    set_error(p, "invalid number");
    return NULL;
}

static json_value *parse_value(parser *p)
{
    skip_space(p);
    if (p->at >= p->end) { set_error(p, "expected value"); return NULL; }
    if (*p->at == '"') { json_value *v = new_value(JSON_STRING); if (v == NULL) { set_error(p, "out of memory"); return NULL; } v->as.string = parse_string(p); if (v->as.string == NULL) { json_free(v); return NULL; } return v; }
    if (*p->at == '{') return parse_object(p);
    if (*p->at == '[') return parse_array(p);
    if (*p->at == '-' || isdigit((unsigned char)*p->at)) return parse_number(p);
    if ((size_t)(p->end - p->at) >= 4 && !memcmp(p->at, "true", 4)) { json_value *v = new_value(JSON_BOOL); if (v == NULL) { set_error(p, "out of memory"); return NULL; } v->as.boolean = 1; p->at += 4; return v; }
    if ((size_t)(p->end - p->at) >= 5 && !memcmp(p->at, "false", 5)) { json_value *v = new_value(JSON_BOOL); if (v == NULL) { set_error(p, "out of memory"); return NULL; } p->at += 5; return v; }
    if ((size_t)(p->end - p->at) >= 4 && !memcmp(p->at, "null", 4)) { json_value *v = new_value(JSON_NULL); if (v == NULL) { set_error(p, "out of memory"); return NULL; } p->at += 4; return v; }
    set_error(p, "unknown value");
    return NULL;
}

int json_parse(const char *input, size_t length, json_value **out, char **error)
{
    parser p = {input, input, input + length, {0}};
    json_value *value;
    if (out == NULL) return 0;
    *out = NULL;
    if (error != NULL) *error = NULL;
    value = parse_value(&p);
    skip_space(&p);
    if (value == NULL || p.at != p.end) {
        if (value != NULL) json_free(value);
        if (p.message[0] == '\0') set_error(&p, "trailing input");
        if (error != NULL) *error = strdup(p.message[0] != '\0' ? p.message : "invalid JSON");
        return 0;
    }
    *out = value;
    return 1;
}

void json_free(json_value *value)
{
    size_t i;
    if (value == NULL) return;
    if (value->type == JSON_STRING || value->type == JSON_NUMBER) free(value->as.string);
    else if (value->type == JSON_ARRAY) { for (i = 0; i < value->as.array.length; i++) json_free(value->as.array.items[i]); free(value->as.array.items); }
    else if (value->type == JSON_OBJECT) { for (i = 0; i < value->as.object.length; i++) { free(value->as.object.members[i].key); json_free(value->as.object.members[i].value); } free(value->as.object.members); }
    free(value);
}

const json_value *json_object_get(const json_value *object, const char *key)
{
    size_t i;
    if (object == NULL || object->type != JSON_OBJECT) return NULL;
    for (i = 0; i < object->as.object.length; i++)
        if (!strcmp(object->as.object.members[i].key, key)) return object->as.object.members[i].value;
    return NULL;
}

int json_get_string(const json_value *value, const char **out)
{
    if (value == NULL || value->type != JSON_STRING || out == NULL) return 0;
    *out = value->as.string;
    return 1;
}

int json_get_u64(const json_value *value, uint64_t *out)
{
    char *end;
    unsigned long long parsed;
    if (value == NULL || value->type != JSON_NUMBER || out == NULL || value->as.number[0] == '-') return 0;
    errno = 0;
    parsed = strtoull(value->as.number, &end, 10);
    if (errno == ERANGE || *end != '\0' || parsed > UINT64_MAX) return 0;
    *out = (uint64_t)parsed;
    return 1;
}
