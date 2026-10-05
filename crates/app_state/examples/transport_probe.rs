//! Round trip Windows ↔ WSL over the stdio of one long-lived process, as the agent would talk (M15).
//!
//! `cargo run --release -p app_state --example transport_probe -- <distro> /mnt/d/cogit/crates/app_state/examples/transport_responder.py`
//! The responder reads a decimal size per line and answers with that many bytes.

use std::io::{BufWriter, Read, Write};
use std::process::{Command, Stdio};
use std::time::Instant;

const RUNS: usize = 50;
const SIZES: [usize; 6] = [100, 1_000, 10_000, 100_000, 1_000_000, 10_000_000];

fn out(line: &str) {
    std::io::stdout()
        .write_all(format!("{line}\n").as_bytes())
        .ok();
}

fn ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (distro, responder) = (&args[0], &args[1]);

    let started = Instant::now();
    let mut child = Command::new("wsl.exe")
        .args(["-d", distro, "--", "python3", "-u", responder])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut input = BufWriter::new(child.stdin.take().ok_or(std::io::ErrorKind::BrokenPipe)?);
    let mut output = child.stdout.take().ok_or(std::io::ErrorKind::BrokenPipe)?;
    let mut buffer = vec![0u8; SIZES[SIZES.len() - 1]];

    let mut call = |size: usize| -> std::io::Result<f64> {
        let started = Instant::now();
        writeln!(input, "{size}")?;
        input.flush()?;
        output.read_exact(&mut buffer[..size])?;
        Ok(ms(started))
    };
    call(1)?;
    out(&format!(
        "spawn + first answer      {:>8.1} ms",
        ms(started)
    ));

    for size in SIZES {
        let mut samples: Vec<f64> = (0..RUNS).map(|_| call(size)).collect::<Result<_, _>>()?;
        samples.sort_by(f64::total_cmp);
        let median = samples[RUNS / 2];
        let p95 = samples[RUNS * 95 / 100];
        let rate = size as f64 / median / 1000.0;
        out(&format!(
            "{size:>10} B  median {median:>8.3} ms  p95 {p95:>8.3} ms  {rate:>7.0} MB/s"
        ));
    }
    drop(input);
    child.wait()?;
    Ok(())
}
