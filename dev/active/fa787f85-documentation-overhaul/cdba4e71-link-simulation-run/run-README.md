# DVB-T2 BICM AWGN Campaign

## Invocation

```
target/release/dvb_t2_awgn_campaign --rate 1/2 --modulation 16qam --esn0-range 5.9:6.2:0.1 --decoder sumproduct --demap exactlogmap --target-errors 50 --max-frames 300 --heartbeat-frames 50 --output-dir dvb_r12_16qam --seed 42
```

## Configuration

- Rate: 1/2
- Modulation: 16qam
- Es/N0 range: 5.90 : 6.20 (4 points)
- Target errors: 50
- Max frames: 300
- Seed: 0x000000000000002a

## Host

- User: vkaskivuo
- System: Linux fraktaali 7.2.6-arch2-1 #1 SMP PREEMPT_DYNAMIC Mon, 14 Sep 2026 22:41:30 +0000 x86_64 GNU/Linux

## Wall-clock

Total: 38.6s (0.6 min)

## Plotting

```bash
python3 dev/benchmarks/dvb_t2_awgn/plot.py \
--curve-csv curve_1_2_16qam.csv \
--reference-toml crates/gf2-coding/data/dvb_t2_tr102831_reference.toml \
--output curve_1_2_16qam.png
```
