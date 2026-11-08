//! Dedicated thread that owns the [`MsiLaptop`] (its COM proxies are not `Send`).

use std::sync::mpsc;
use std::thread;

use crate::{Error, MsiLaptop, Result};

type Job = Box<dyn FnOnce(&MsiLaptop) + Send>;

/// Cheap, cloneable handle to the hardware thread.
#[derive(Clone, Debug)]
pub struct HwWorker {
    tx: mpsc::Sender<Job>,
}

impl HwWorker {
    /// Spawns the thread and connects to `MSI_ACPI`; returns the connection error if any.
    pub fn spawn() -> Result<Self> {
        let (tx, rx) = mpsc::channel::<Job>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();
        thread::Builder::new()
            .name("msi-acpi".into())
            .spawn(move || {
                let laptop = match MsiLaptop::connect() {
                    Ok(laptop) => laptop,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));
                for job in rx {
                    job(&laptop);
                }
            })
            .map_err(|e| Error::Unavailable(format!("cannot spawn hardware thread: {e}")))?;
        ready_rx.recv().map_err(|_| Error::WorkerStopped)??;
        Ok(Self { tx })
    }

    /// Runs `f` on the hardware thread and blocks until it returns.
    ///
    /// Call from a background executor, never from a UI thread.
    pub fn run<R: Send + 'static>(
        &self,
        f: impl FnOnce(&MsiLaptop) -> Result<R> + Send + 'static,
    ) -> Result<R> {
        let (tx, rx) = mpsc::channel();
        self.tx
            .send(Box::new(move |laptop| {
                let _ = tx.send(f(laptop));
            }))
            .map_err(|_| Error::WorkerStopped)?;
        rx.recv().map_err(|_| Error::WorkerStopped)?
    }
}
