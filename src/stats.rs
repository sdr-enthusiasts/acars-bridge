// Copyright (c) 2024-2026 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc::Receiver;
// A struct to hold the stats

pub struct Stats {
    total_all_time: Arc<AtomicU64>,
    total_since_last: Arc<AtomicU64>,
    receiver: Receiver<u8>,
}

impl Stats {
    #[must_use]
    pub fn new(receiver: Receiver<u8>) -> Self {
        Self {
            total_all_time: Arc::new(AtomicU64::new(0)),
            total_since_last: Arc::new(AtomicU64::new(0)),
            receiver,
        }
    }

    pub fn run(mut self, print_interval: u64) {
        // clone the Arcs so we can pass them to the print_stats function
        let total_all_time_context = self.total_all_time.clone();
        let total_since_last_context = self.total_since_last.clone();

        trace!("[STATS] Starting stats thread");
        tokio::spawn(async move {
            print_stats_to_console(
                total_all_time_context,
                total_since_last_context,
                print_interval,
            )
            .await;
        });

        tokio::spawn(async move {
            self.watch_message_queue().await;
        });
    }

    async fn watch_message_queue(&mut self) {
        while self.receiver.recv().await.is_some() {
            trace!("[STATS] Received message from queue");
            self.increment();
        }
        // All Senders have been dropped. This should not happen under normal
        // operation because main retains a master Sender, but if it does we
        // exit cleanly instead of spinning on the closed channel.
        warn!("[STATS] Stats channel closed (all senders dropped); exiting stats watcher");
    }

    fn increment(&self) {
        self.total_all_time.fetch_add(1, Ordering::Relaxed);
        self.total_since_last.fetch_add(1, Ordering::Relaxed);
    }

    #[must_use]
    pub fn get_total_all_time(&self) -> u64 {
        self.total_all_time.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn get_total_last_interval(&self) -> u64 {
        self.total_since_last.load(Ordering::Relaxed)
    }
}

async fn print_stats_to_console(
    total_all_time_context: Arc<AtomicU64>,
    total_since_last_context: Arc<AtomicU64>,
    print_interval: u64,
) {
    // print interval is in minutes, so we need to convert it to seconds.
    // saturating_mul guards against u64 overflow if the user passes a
    // pathological value (a debug-mode panic / release-mode wrap).
    let print_interval_in_seconds = print_interval.saturating_mul(60);

    // tokio::time::interval gives drift-free ticks: each tick fires at a
    // multiple of the period from the start instant, so time spent printing
    // and resetting counters does not push successive intervals later. The
    // first tick fires immediately, so skip it.
    let mut ticker =
        tokio::time::interval(tokio::time::Duration::from_secs(print_interval_in_seconds));
    ticker.tick().await;

    loop {
        ticker.tick().await;
        let total_all_time = total_all_time_context.load(Ordering::Relaxed);
        // Atomically swap the per-interval counter to 0 so increments that
        // happen between the read and the reset are not lost.
        let total_since_last = total_since_last_context.swap(0, Ordering::Relaxed);

        info!("[STATS] Total since container start: {total_all_time}");
        info!(
            "[STATS] Total in the last {} minute{}: {}",
            print_interval,
            if print_interval > 1 { "s" } else { "" },
            total_since_last
        );
    }
}
