#!/usr/bin/env bash
# Append the exact argv at a logical campaign or profile dispatch boundary.
record_invocation() {
    local output="$1" kind="$2"
    shift 2
    {
        printf '%s\t' "${kind}"
        printf '%q ' "$@"
        printf '\n'
    } >>"${output}"
}
