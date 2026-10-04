RUSTUP_TOOLCHAIN=1.95.0 charon cargo \
  --preset aeneas \
  --start-from 'seam_shape::driver::route_concrete' \
  --opaque 'seam_shape::field' \
  --dest-file "$PWD"/dev/archive/6dc81018-field-capability-dispatch/active/7d7c647c/probes/M5_seam_shape.llbc \
  -- --manifest-path "$PWD"/dev/archive/6dc81018-field-capability-dispatch/active/7d7c647c/probes/seam-shape/Cargo.toml
