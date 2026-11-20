use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use ureq::unversioned::transport::{Buffers, ConnectionDetails, Connector, NextTimeout, Transport};

const POLL: Duration = Duration::from_millis(100);
const IDLE: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub(crate) struct CancelConnector(pub(crate) Arc<AtomicBool>);

impl Connector<Box<dyn Transport>> for CancelConnector {
    type Out = CancelTransport;

    fn connect(
        &self,
        _: &ConnectionDetails<'_>,
        chained: Option<Box<dyn Transport>>,
    ) -> Result<Option<Self::Out>, ureq::Error> {
        Ok(chained.map(|inner| CancelTransport {
            inner,
            quit: Arc::clone(&self.0),
        }))
    }
}

#[derive(Debug)]
pub(crate) struct CancelTransport {
    inner: Box<dyn Transport>,
    quit: Arc<AtomicBool>,
}

impl CancelTransport {
    fn check(&self) -> Result<(), ureq::Error> {
        if self.quit.load(Ordering::Relaxed) {
            return Err(io::Error::other("Request canceled").into());
        }
        Ok(())
    }
}

impl Transport for CancelTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        self.inner.buffers()
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), ureq::Error> {
        self.check()?;
        self.inner.transmit_output(amount, timeout)
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, ureq::Error> {
        let limit = timeout
            .not_zero()
            .map_or(IDLE, |duration| (*duration).min(IDLE));
        let started = Instant::now();
        loop {
            self.check()?;
            let remaining = limit.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(ureq::Error::Timeout(timeout.reason));
            }
            let tick = NextTimeout {
                after: remaining.min(POLL).into(),
                reason: timeout.reason,
            };
            match self.inner.await_input(tick) {
                Err(ureq::Error::Timeout(_)) => {}
                result => return result,
            }
        }
    }

    fn is_open(&mut self) -> bool {
        self.inner.is_open()
    }

    fn is_tls(&self) -> bool {
        self.inner.is_tls()
    }
}
