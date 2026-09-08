# Paused exploratory attempt: asymmetric matrix setup

The DVB cell completed and retained decoder quality. The NR session was
cancelled while waiting for the mutex, before starting another measurement.
This attempt is not finalized and is not resolution evidence for confirmation.

Inspection found that gf2 cloned a prebuilt parity-check matrix inside timing,
whereas AFF3CT prepared its matrix from the AList inside timing. The shared
measurement contract requires setup and table preparation in whole-consumer
cells. The `2026-09-08-r3` replacement calls gf2's native table-driven matrix
constructor inside each timed call. These earlier samples remain visible and
are not silently reused under the corrected boundary.
