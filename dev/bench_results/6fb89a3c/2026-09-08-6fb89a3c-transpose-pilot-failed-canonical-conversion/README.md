# Rejected transpose pilot: conversion-cost canonicalization

This preserved campaign is negative protocol evidence, not a benchmark receipt.
The runner completed the two fixed 64x64 cells, resumed, and then rejected the
first 63x63 whole-consumer result because the Rust arm serialized the typed
conversion-cost object through `serde_json::Value`. That map's key order did not
match `ConversionCosts`, so the strict child-v2 canonical decoder failed closed.

No receipt was finalized and no performance number from this directory is used
in the findings or as resolution evidence. The harness uses a typed conversion
record and the framing preflight exercises whole-consumer results before the
accepted campaign.
