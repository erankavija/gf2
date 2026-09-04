#!/usr/bin/env bash
# Lead-side B4 retry (owner decision, session 8): GUAVA BCHCode(65535,1,25,GF(2)) with a large heap,
# bounded to 20 min wall and 52 GB by a user scope; RSS sampled every 5 s.
SP=/tmp/claude-1000/-home-vkaskivuo-Projects-gf2/b6ff6d78-3691-40ce-9546-069e618e637c/scratchpad
OUT=$SP/gap-b4
mkdir -p $OUT
echo "start $(date -u +%Y-%m-%dT%H:%M:%SZ)" > $OUT/timeline.txt
command free -m | awk 'NR==2{printf "free-before total=%d used=%d avail=%d\n",$2,$3,$7}' >> $OUT/timeline.txt
systemd-run --user --scope -p MemoryMax=52G --unit=gf2-gap-b4-$$ \
  timeout 1200 gap -q -A -T -o 50g -c 'LoadPackage("guava");; t0:=Runtime();; r:=CALL_WITH_CATCH(BCHCode,[65535,1,25,GF(2)]);; Print("built=",r[1],"\n");; if r[1] then Print("dim=",Dimension(r[2]),"\n"); else Print("error=",r[2],"\n"); fi;; Print("cpu_ms=",Runtime()-t0,"\n");; Print("peak_rss_kib=",Filtered(Concatenation(Filtered(SplitString(StringFile("/proc/self/status"),"\n"),l->PositionSublist(l,"VmHWM:")<>fail)),c->c in "0123456789"),"\n");; QUIT;' \
  > $OUT/gap.out 2> $OUT/gap.err &
GP=$!
# RSS sampler
( while kill -0 $GP 2>/dev/null; do
    pid=$(pgrep -x gap | head -1)
    if [ -n "$pid" ]; then echo "$(date -u +%H:%M:%S) rss_kib=$(awk '/VmRSS/{print $2}' /proc/$pid/status 2>/dev/null) hwm_kib=$(awk '/VmHWM/{print $2}' /proc/$pid/status 2>/dev/null)"; fi
    sleep 5
  done ) >> $OUT/rss.log 2>&1 &
wait $GP; rc=$?
echo "exit=$rc end $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> $OUT/timeline.txt
echo $rc > $OUT/done
