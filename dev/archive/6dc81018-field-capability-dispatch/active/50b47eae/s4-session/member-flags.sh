# member_flags <j> -> prints RUSTFLAGS string (may be empty)
member_flags() {
  local j="$1" e g parts payload
  e=$(( j / 2 ))
  g=$(( ( (j / 4) + j ) % 2 ))
  parts=""
  if [ "$g" -eq 1 ]; then parts="-C link-dead-code"; fi
  if [ "$e" -gt 0 ]; then
    payload=$(( 2 * (20 + 32 * e) ))
    local zeros
    zeros=$(printf '%0*d' "$payload" 0)
    if [ -n "$parts" ]; then parts="$parts -C link-arg=-Wl,--build-id=0x$zeros"
    else parts="-C link-arg=-Wl,--build-id=0x$zeros"; fi
  fi
  printf '%s' "$parts"
}
# member_label <j> -> abbreviated flags for the ledger
member_label() {
  local j="$1" e g parts payload
  e=$(( j / 2 ))
  g=$(( ( (j / 4) + j ) % 2 ))
  parts=""
  if [ "$g" -eq 1 ]; then parts="-C link-dead-code"; fi
  if [ "$e" -gt 0 ]; then
    payload=$(( 2 * (20 + 32 * e) ))
    if [ -n "$parts" ]; then parts="$parts -C link-arg=-Wl,--build-id=0x<$payload zeros>"
    else parts="-C link-arg=-Wl,--build-id=0x<$payload zeros>"; fi
  fi
  if [ -z "$parts" ]; then printf '(none)'; else printf '%s' "$parts"; fi
}
