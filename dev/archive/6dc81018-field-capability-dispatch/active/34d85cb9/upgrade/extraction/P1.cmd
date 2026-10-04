RUSTUP_TOOLCHAIN=1.95.0 /data/aeneas-upgrade-34d85cb9/aeneas/charon/bin/charon rustc \
  --preset aeneas \
  --dest-file dev/archive/6dc81018-field-capability-dispatch/active/34d85cb9/upgrade/extraction/P1_probe.llbc \
  -- --crate-type lib --edition 2021 dev/archive/6dc81018-field-capability-dispatch/active/34d85cb9/extraction/probe-P1.rs
