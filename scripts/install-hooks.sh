#!/bin/sh
# Points this clone and its worktrees at the versioned hooks.
exec git -C "$(dirname "$0")/.." config core.hooksPath .githooks
