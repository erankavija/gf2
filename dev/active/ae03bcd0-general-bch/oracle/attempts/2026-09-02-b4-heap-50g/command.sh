#!/usr/bin/env bash
# Lead-run bounded GUAVA attempt at corpus row B4 (2026-09-02 15:59:28Z-16:08:45Z, host fraktaali,
# main at 7389e716 for the record; the attempt reads no repository source).
# Heap 50 GiB is the largest the host could back: 48.6 GiB was available at launch.
systemd-run --user --scope -p MemoryMax=52G \
  timeout 1200 gap -q -A -T -o 50g -c 'LoadPackage("guava");; t0:=Runtime();; r:=CALL_WITH_CATCH(BCHCode,[65535,1,25,GF(2)]);; Print("built=",r[1],"\n");; if r[1] then Print("dim=",Dimension(r[2]),"\n"); else Print("error=",r[2],"\n"); fi;; Print("cpu_ms=",Runtime()-t0,"\n");; QUIT;' \
  > gap.out 2> gap.err
# rss.log: VmRSS/VmHWM of the gap process sampled every 5 s from /proc/<pid>/status.
