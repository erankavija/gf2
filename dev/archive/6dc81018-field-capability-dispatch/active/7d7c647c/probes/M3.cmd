RUSTUP_TOOLCHAIN=1.95.0 charon cargo \
  --preset aeneas \
  --start-from 'seam_shape::driver::route_seam' \
  --dest-file "$PWD"/dev/active/7d7c647c/probes/M3_seam_shape.llbc \
  -- --manifest-path "$PWD"/dev/active/7d7c647c/probes/seam-shape/Cargo.toml
