//! Builds one immutable LDPC decoder input bundle (jit:c077a88b).
//!
//! The bundle fixes the parity-check matrix, the transmitted codewords and
//! the recorded channel LLRs that every arm of the survey decodes. The
//! parity-check matrix comes from the workspace `export_alist` binary, the
//! single construction site of the comparison AList, so this binary never
//! re-implements the AList writer.
//!
//! # Usage
//!
//! ```bash
//! ldpc-make-inputs --code dvb-t2-r12 --alist <exported.alist> \
//!     --esn0-db -1.2 --frames 128 --seed 42 --codeword-source both \
//!     --punctured-prefix 0 --output <bundle-dir>
//! ```

use gf2_coding::ldpc::{LdpcCode, LdpcEncoder};
use gf2_coding::traits::BlockEncoder;
use gf2_core::rng::Lcg;
use gf2_core::BitVec;
use gf2_sim::testutil::{AwgnLlrSource, ComparisonCode};
use ldpc_survey::{
    sha256_file, sha256_hex, BundleManifest, CodewordSource, ALIST_FILE, BUNDLE_SCHEMA,
    CODEWORDS_FILE, LLRS_FILE, MANIFEST_FILE,
};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

struct Args {
    code: String,
    alist: PathBuf,
    esn0_db: f64,
    frames: usize,
    seed: u64,
    codeword_source: CodewordSource,
    punctured_prefix: usize,
    output: PathBuf,
}

fn parse_args() -> Result<Args, String> {
    let mut code = None;
    let mut alist = None;
    let mut esn0_db = None;
    let mut frames = 128usize;
    let mut seed = 42u64;
    let mut codeword_source = CodewordSource::Both;
    let mut punctured_prefix = 0usize;
    let mut output = None;
    let mut argv = std::env::args().skip(1);
    while let Some(flag) = argv.next() {
        let mut value = || argv.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--code" => code = Some(value()?),
            "--alist" => alist = Some(PathBuf::from(value()?)),
            "--esn0-db" => esn0_db = Some(value()?.parse::<f64>().map_err(|e| e.to_string())?),
            "--frames" => frames = value()?.parse::<usize>().map_err(|e| e.to_string())?,
            "--seed" => seed = value()?.parse::<u64>().map_err(|e| e.to_string())?,
            "--codeword-source" => {
                codeword_source = match value()?.as_str() {
                    "all-zero" => CodewordSource::AllZero,
                    "random" => CodewordSource::Random,
                    "both" => CodewordSource::Both,
                    other => return Err(format!("unknown --codeword-source {other}")),
                }
            }
            "--punctured-prefix" => {
                punctured_prefix = value()?.parse::<usize>().map_err(|e| e.to_string())?
            }
            "--output" => output = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(Args {
        code: code.ok_or("--code is required")?,
        alist: alist.ok_or("--alist is required")?,
        esn0_db: esn0_db.ok_or("--esn0-db is required")?,
        frames,
        seed,
        codeword_source,
        punctured_prefix,
        output: output.ok_or("--output is required")?,
    })
}

/// Transmitted codeword of frame `index` under the declared source.
fn codeword(
    source: CodewordSource,
    index: usize,
    code: &LdpcCode,
    encoder: Option<&LdpcEncoder>,
    messages: &mut Lcg,
) -> BitVec {
    let random = match source {
        CodewordSource::AllZero => false,
        CodewordSource::Random => true,
        CodewordSource::Both => index % 2 == 1,
    };
    if !random {
        return BitVec::zeros(code.n());
    }
    let encoder = encoder.expect("a random codeword needs an encoder");
    let mut message = BitVec::with_capacity(code.k());
    let mut remaining = code.k();
    while remaining > 0 {
        let mut word = messages.next_u64();
        for _ in 0..remaining.min(64) {
            message.push_bit(word & 1 == 1);
            word >>= 1;
        }
        remaining -= remaining.min(64);
    }
    encoder.encode(&message)
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let comparison = ComparisonCode::parse(&args.code)?;
    let code = comparison.build();
    let n = code.n();
    let k = code.k();
    let m = code.m();
    if args.punctured_prefix >= n {
        return Err(format!(
            "--punctured-prefix {} is not below n = {n}",
            args.punctured_prefix
        ));
    }
    let needs_encoder = !matches!(args.codeword_source, CodewordSource::AllZero);
    let encoder = needs_encoder.then(|| LdpcEncoder::new(code.clone()));

    let sigma = (1.0 / (2.0 * 10_f64.powf(args.esn0_db / 10.0))).sqrt();
    // One stream for the transmitted messages and one for the channel, so a
    // change of codeword source cannot reshape the channel noise of the
    // all-zero frames.
    let mut messages = Lcg::new(args.seed ^ 0x6C64_7063_6D73_6773);
    let mut channel = AwgnLlrSource::new(args.seed);

    let mut codewords = vec![0u8; args.frames * n];
    let mut llrs = vec![0u8; 4 * args.frames * n];
    for frame in 0..args.frames {
        let transmitted = codeword(
            args.codeword_source,
            frame,
            &code,
            encoder.as_ref(),
            &mut messages,
        );
        if !code.is_valid_codeword(&transmitted) {
            return Err(format!("frame {frame} transmitted a non-codeword"));
        }
        let frame_llrs = channel.frame_for_codeword(&transmitted, sigma);
        for position in 0..n {
            codewords[frame * n + position] = u8::from(transmitted.get(position));
            let value = if position < args.punctured_prefix {
                0.0_f32
            } else {
                frame_llrs[position].value()
            };
            let bytes = value.to_le_bytes();
            llrs[4 * (frame * n + position)..4 * (frame * n + position) + 4]
                .copy_from_slice(&bytes);
        }
    }

    fs::create_dir_all(&args.output).map_err(|e| e.to_string())?;
    let alist_target = args.output.join(ALIST_FILE);
    fs::copy(&args.alist, &alist_target).map_err(|e| e.to_string())?;
    fs::write(args.output.join(CODEWORDS_FILE), &codewords).map_err(|e| e.to_string())?;
    fs::write(args.output.join(LLRS_FILE), &llrs).map_err(|e| e.to_string())?;

    let manifest = BundleManifest {
        schema: BUNDLE_SCHEMA.to_owned(),
        code: args.code.clone(),
        n,
        k,
        m,
        nnz: (0..m)
            .map(|row| code.parity_check_matrix().row_iter(row).count())
            .sum(),
        h_sha256: sha256_file(&alist_target).map_err(|e| e.to_string())?,
        codewords_sha256: sha256_hex(&codewords),
        llrs_sha256: sha256_hex(&llrs),
        frames: args.frames,
        seed: args.seed,
        esn0_db: args.esn0_db,
        sigma,
        codeword_source: args.codeword_source,
        punctured_prefix: args.punctured_prefix,
    };
    let mut encoded = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    encoded.push(b'\n');
    fs::write(args.output.join(MANIFEST_FILE), &encoded).map_err(|e| e.to_string())?;
    println!("{}", args.output.join(MANIFEST_FILE).display());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ldpc-make-inputs: {error}");
            ExitCode::FAILURE
        }
    }
}
