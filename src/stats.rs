use iced::{
    Subscription,
    futures::{SinkExt, Stream},
    stream,
};
use std::time::Duration;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

const INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy)]
pub struct Snapshot {
    pub cpu_percent: Option<f32>,
    pub memory_bytes: u64,
}

impl Snapshot {
    pub fn label(&self) -> String {
        let cpu = self
            .cpu_percent
            .map_or_else(|| "…".into(), |cpu| format!("{cpu:.1}%"));
        format!(
            "CPU {cpu} · RAM {:.0} MiB",
            self.memory_bytes as f64 / 1_048_576.0
        )
    }
}

pub fn subscription(enabled: bool) -> Subscription<Option<Snapshot>> {
    if enabled {
        Subscription::run(samples)
    } else {
        Subscription::none()
    }
}

fn samples() -> impl Stream<Item = Option<Snapshot>> {
    stream::channel(1, async |mut output| {
        let mut monitor = Monitor::new();
        loop {
            // OS process queries run outside the UI/event-loop thread. Keep
            // the monitor across samples because CPU usage needs time deltas.
            let Ok((updated, snapshot)) = tokio::task::spawn_blocking(move || {
                let snapshot = monitor.sample();
                (monitor, snapshot)
            })
            .await
            else {
                let _ = output.send(None).await;
                return;
            };
            monitor = updated;
            if output.send(snapshot).await.is_err() {
                return;
            }
            tokio::time::sleep(INTERVAL).await;
        }
    })
}

struct Monitor {
    system: System,
    pid: Pid,
    primed: bool,
}

impl Monitor {
    fn new() -> Self {
        Self {
            system: System::new(),
            pid: Pid::from_u32(std::process::id()),
            primed: false,
        }
    }

    fn sample(&mut self) -> Option<Snapshot> {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[self.pid]),
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .without_tasks(),
        );
        let process = self.system.process(self.pid)?;
        let snapshot = Snapshot {
            cpu_percent: self.primed.then(|| process.cpu_usage()),
            memory_bytes: process.memory(),
        };
        self.primed = true;
        Some(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_current_process_and_primes_cpu_measurement() {
        let mut monitor = Monitor::new();
        let first = monitor
            .sample()
            .expect("current process should be available");
        assert!(first.memory_bytes > 0);
        assert!(first.cpu_percent.is_none());
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL + Duration::from_millis(50));
        let second = monitor.sample().unwrap();
        let cpu = second
            .cpu_percent
            .expect("second sample should have CPU usage");
        assert!(cpu.is_finite() && cpu >= 0.0);
        assert!(second.memory_bytes > 0);
        assert_eq!(monitor.system.processes().len(), 1);
    }
}
