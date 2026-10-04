RUSTUP_TOOLCHAIN=1.95.0 charon cargo \
  --preset aeneas \
  --start-from 'seam_shape::driver::route_hooked' \
  --dest-file "$PWD"/dev/archive/6dc81018-field-capability-dispatch/active/7d7c647c/probes/M4_seam_shape.llbc \
  -- --manifest-path "$PWD"/dev/archive/6dc81018-field-capability-dispatch/active/7d7c647c/probes/seam-shape/Cargo.toml
