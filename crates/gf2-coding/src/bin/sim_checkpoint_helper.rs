//! Helper binary for the subprocess-SIGINT resume test
//! (`test_resume_after_interrupt`): runs a seeded Hamming(7,4)/OrbGrand
//! BPSK-AWGN simulation that checkpoints into the directory given as its one
//! argument. It exits 0 after a complete run, 1 when interrupted after
//! flushing a checkpoint, and 2 on a usage error or without the
//! `sim-observability` feature; a rerun with the same directory resumes.

fn main() {
    #[cfg(feature = "sim-observability")]
    {
        use gf2_coding::grand::{OrbGrand, OrbGrandConfig};
        use gf2_coding::linear::LinearBlockCode;
        use gf2_coding::simulation::{
            BpskAwgnChannel, ChannelModel, SimulationConfig, SimulationRunner,
        };
        use gf2_coding::Llr;
        use gf2_core::BitVec;
        use std::path::PathBuf;
        use std::time::Duration;

        // The per-frame sleep keeps the run alive long enough for the test to deliver SIGINT.
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
                self.inner
                    .transmit_and_demodulate(bits, eb_n0_db, rate, rng)
            }
        }

        let args: Vec<String> = std::env::args().collect();
        if args.len() != 2 {
            eprintln!("Usage: sim_checkpoint_helper <checkpoint_dir>");
            std::process::exit(2);
        }
        let ckpt_dir = PathBuf::from(&args[1]);

        std::fs::create_dir_all(&ckpt_dir).unwrap_or_else(|e| {
            eprintln!("Cannot create checkpoint dir: {e}");
            std::process::exit(2);
        });

        let code = LinearBlockCode::hamming(3);
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
            heartbeat_every_frames: Some(5),
        };

        SimulationRunner::run_coded(&code, &decoder, &channel, &config);
    }

    #[cfg(not(feature = "sim-observability"))]
    {
        eprintln!("sim_checkpoint_helper requires the sim-observability feature");
        std::process::exit(2);
    }
}
