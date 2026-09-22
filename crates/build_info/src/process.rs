use std::io::{Error, ErrorKind, Read as _};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Standard output of a command that must succeed within `limit`, or it is killed.
pub fn output_within(mut command: Command, limit: Duration) -> std::io::Result<Vec<u8>> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::other("no standard output"))?;
    // Read on the side: a child that fills the pipe would otherwise never exit.
    let reader = std::thread::spawn(move || {
        let mut out = Vec::new();
        stdout.read_to_end(&mut out).map(|_| out)
    });

    let deadline = Instant::now() + limit;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::new(
                ErrorKind::TimedOut,
                format!("gave up after {} s", limit.as_secs()),
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    let out = reader
        .join()
        .map_err(|_| Error::other("the reader thread panicked"))??;
    if status.success() {
        Ok(out)
    } else {
        Err(Error::other(format!("exited with {status}")))
    }
}
