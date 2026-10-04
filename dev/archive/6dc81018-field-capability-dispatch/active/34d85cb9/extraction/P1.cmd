RUSTUP_TOOLCHAIN=1.95.0 charon rustc \
  --preset aeneas \
  --dest-file dev/active/34d85cb9/extraction/P1_probe.llbc \
  -- --crate-type lib --edition 2021 dev/active/34d85cb9/extraction/probe-P1.rs
