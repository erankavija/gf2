//! Helper binary for the subprocess-SIGINT resume integration test
//! (`test_resume_after_interrupt`).
//!
//! # Contract
//!
//! Accepts exactly one positional argument — a path to a directory that will
//! hold checkpoint files and a `results.csv` output.  Runs a tiny, fully
//! deterministic simulation:
//!
//! - Code: Hamming(7,4), decoded by OrbGrand
//! - Channel: BPSK/AWGN at 0.0 dB (guaranteed frame errors)
//! - Config: seed=12345, min_errors=10, max_frames=1000,
//!   heartbeat_every_frames=5
//!
//! On a normal run the binary exits with code 0 and `results.csv` contains
//! one row.  When interrupted mid-run (SIGINT/SIGTERM), `ctrlc` sets the
//! interrupt flag, the runner flushes a partial checkpoint, and the binary
//! exits with code 1.  Re-running with the same argument resumes from the
//! checkpoint.
//!
//! The integration test spawns this binary, delivers SIGINT, then re-runs
//! and verifies that the final CSV matches a reference uninterrupted run.
//!
//! # Feature gate
//!
//! Only compiled when `sim-observability` is enabled (the default).
//! Without the feature the binary prints a message and exits with code 2.

fn main() {
    #[cfg(feature = "sim-observability")]
    {
        use gf2_coding::grand::{OrbGrand, OrbGrandConfig};
        use gf2_coding::linear::LinearBlockCode;
        use gf2_coding::simulation::{BpskAwgnChannel, ChannelModel, SimulationConfig, SimulationRunner};
        use gf2_coding::Llr;
        use gf2_core::BitVec;
        use std::path::PathBuf;
        use std::time::Duration;

        // Per-frame sleep wrapper so the test's 50 ms SIGINT poll arrives while
        // the simulation is still running.  Without it, Hamming(7,4) at BLER=1.0
        // finishes all 10 frames in < 1 ms — before the polling loop can fire.
        // 20 ms × 10 frames = 200 ms total; heartbeat at frame 5 = 100 ms, leaving
        // a 100 ms window for the test to send SIGINT.
        struct ThrottledChannel {
            inner: BpskAwgnChannel,
            delay: Duration,
        }
        impl ChannelModel for ThrottledChannel {
            fn transmit_and_demodulate<R: rand::Rng>(
                &self,
                bits: &BitVec,
                eb_n0_db: f64,
                rate: f64,
                rng: &mut R,
            ) -> Vec<Llr> {
                std::thread::sleep(self.delay);
                self.inner.transmit_and_demodulate(bits, eb_n0_db, rate, rng)
            }
        }

        let args: Vec<String> = std::env::args().collect();
        if args.len() != 2 {
            eprintln!("Usage: sim_checkpoint_helper <checkpoint_dir>");
            std::process::exit(2);
        }
        let ckpt_dir = PathBuf::from(&args[1]);

        // Ensure the output directory exists.
        std::fs::create_dir_all(&ckpt_dir).unwrap_or_else(|e| {
            eprintln!("Cannot create checkpoint dir: {e}");
            std::process::exit(2);
        });

        let code = LinearBlockCode::hamming(3); // Hamming(7,4)
        let h = code
            .parity_check()
            .expect("Hamming code must have H")
            .clone();
        let decoder = OrbGrand::new(h, OrbGrandConfig::default());
        let channel = ThrottledChannel {
            inner: BpskAwgnChannel,
            delay: Duration::from_millis(20),
        };

        let config = SimulationConfig {
            eb_n0_range_db: vec![0.0], // low SNR -> fast frame errors
            min_errors: 10,
            max_frames: 1000,
            max_decoder_iterations: 50,
            rng_seed: Some(12345),
            output_path: Some(ckpt_dir.join("results.csv")),
            checkpoint_dir: Some(ckpt_dir.clone()),
            tracing_log_path: None,
            heartbeat_every_frames: Some(5), // write checkpoint every 5 frames
        };

        SimulationRunner::run_coded(&code, &decoder, &channel, &config);
    }

    #[cfg(not(feature = "sim-observability"))]
    {
        eprintln!("sim_checkpoint_helper requires the sim-observability feature");
        std::process::exit(2);
    }
}
