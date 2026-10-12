#!/bin/sh
# Linker driver of the arms build: `cc`, with the linker writing beside its
# output the make rules of the files it read. rustc takes the `cc` argument
# convention from the `-cc` name.
previous=
output=
for argument do
    [ "$previous" = -o ] && output=$argument
    previous=$argument
done
[ -n "$output" ] || exec cc "$@"
exec cc "$@" -Xlinker "--dependency-file=$output.link.d"
