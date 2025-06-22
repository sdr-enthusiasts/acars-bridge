// Copyright (c) 2024 Fred Clausen
//
// Licensed under the MIT license: https://opensource.org/licenses/MIT
// Permission is granted to use, copy, modify, and redistribute the work.
// Full license information available in the project LICENSE file.

use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
// A struct to hold the stats

pub struct Stats {
    pub total_all_time: Arc<Mutex<u64>>,
    pub total_since_last: Arc<Mutex<u64>>,
    receiver: Receiver<u8>,
}

impl Stats {
    #[must_use]
    pub fn new(receiver: Receiver<u8>) -> Self {
        // wrap the stats in an Arc<Mutex<Stats>> to allow for multiple threads to access it
        Self {
            total_all_time: Arc::new(Mutex::new(0)),
            total_since_last: Arc::new(Mutex::new(0)),
            receiver,
        }
    }

    pub fn run(mut self, print_interval: u64) {
        // clone the Arc<Mutex<Stats>> so we can pass it to the print_stats function
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

    pub async fn watch_message_queue(&mut self) {
        loop {
            match self.receiver.recv().await {
                Some(_) => {
                    trace!("[STATS] Received message from queue");
                    self.increment().await;
                }
                None => {
                    error!("[STATS] Error receiving message from queue");
                }
            }
        }
    }

    pub async fn increment(&mut self) {
        *self.total_all_time.lock().await += 1;
        *self.total_since_last.lock().await += 1;
    }

    pub async fn get_total_all_time(&self) -> u64 {
        *self.total_all_time.lock().await
    }

    pub async fn get_total_last_interval(&self) -> u64 {
        *self.total_since_last.lock().await
    }
}

pub async fn print_stats_to_console(
    total_all_time_context: Arc<Mutex<u64>>,
    total_since_last_context: Arc<Mutex<u64>>,
    print_interval: u64,
) {
    loop {
        // print interval is in minutes, so we need to convert it to seconds
        let print_interval_in_seconds = print_interval * 60;
        tokio::time::sleep(tokio::time::Duration::from_secs(print_interval_in_seconds)).await;
        let total_all_time = *total_all_time_context.lock().await;
        let total_since_last = *total_since_last_context.lock().await;

        info!("[STATS] Total since container start: {total_all_time}");
        info!(
            "[STATS] Total in the last {} minute{}: {}",
            print_interval,
            if print_interval > 1 { "s" } else { "" },
            total_since_last
        );

        // set the total_last_interval to 0
        *total_since_last_context.lock().await = 0;

        // FIXME: Can we have a situation where the mutex is written to after we've read the value but before we've reset it?
    }
}
