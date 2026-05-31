# acars-bridge

Small utility to bridge from the SDR-E ACARS/HFDL/VDL2 decoder containers to acars_router.

## Usage

acars-bridge can be used to connect to a running SDR-E container and bridge the output to acars_router. It can accept input over UDP, TCP or ZMQ, and optionally output over UDP, TCP or ZMQ. It is a replacement for the TCP/UDP internal wiring that was used in the containers to both send data to acars_router (in the case of `acarsdec`/`vdlm2dec`) and to output stats to container logs.

Note, bridge is only set up to actively connect to the source/destination, not to listen for incoming connections.

If no destination is configured, the bridge still runs the input side and periodically logs receive statistics; this is useful for quickly verifying that a decoder is producing data.

### Command line flags

Every flag may also be supplied via the matching environment variable.

| Flag                     | Env var                   | Description                                                                                                                                                | Default |
| ------------------------ | ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| `--log-level`            | `AB_LOG_LEVEL`            | Log level. `info`, `debug`, and `trace` are valid inputs.                                                                                                  | `info`  |
| `--source-host`          | `AB_SOURCE`               | Hostname or IP address where the decoder is sending data from. **Required.**                                                                               | _unset_ |
| `--source-port`          | `AB_SOURCE_PORT`          | Port where the decoder is sending data from. **Required.**                                                                                                 | _unset_ |
| `--source-protocol`      | `AB_SOURCE_PROTOCOL`      | Protocol to use for the source. `udp`, `tcp`, or `zmq`. **Required.**                                                                                      | _unset_ |
| `--destination-host`     | `AB_DESTINATION`          | Hostname or IP address where acars_router is running. Optional; enables the output side.                                                                   | _unset_ |
| `--destination-port`     | `AB_DESTINATION_PORT`     | Port where acars_router is running. Required if `--destination-host` is set.                                                                               | _unset_ |
| `--destination-protocol` | `AB_DESTINATION_PROTOCOL` | Protocol to use for the destination. `udp`, `tcp`, or `zmq`. Required if `--destination-host` is set.                                                      | _unset_ |
| `--stat-interval`        | `AB_STAT_INTERVAL`        | Interval in minutes to output stats to the log. Must be `>= 1`.                                                                                            | `5`     |
| `--channel-capacity`     | `AB_CHANNEL_CAPACITY`     | Capacity of the internal mpsc channels (input→output bridge and stats). Higher values absorb more burstiness before backpressure kicks in. Must be `>= 1`. | `1024`  |

### Resilience

Each side (input, output) runs under its own supervisor task, and stats runs as its own task. Behavior on exit:

- **Input supervisor**: any inner exit (graceful peer close or error) triggers a reconnect with exponential backoff (1s → 60s, reset after 60s of stable runtime). Decoders may restart, and the bridge should reconnect to them automatically.
- **Output supervisor**: an I/O error triggers a reconnect with the same exponential backoff. A graceful exit (only possible when the bridge channel has been closed during shutdown) is terminal — the supervisor does not restart.

### Graceful shutdown

acars-bridge handles `SIGINT` (Ctrl-C) and `SIGTERM` with a coordinated drain:

1. The shutdown signal cancels the input supervisor; its current connection attempt or read loop is aborted, and it exits its loop without restarting.
2. main joins the input supervisor, then drops its master clone of the bridge channel `Sender`. The output's `recv()` continues to return queued messages until the channel is empty, at which point it returns `None` and `watch_queue` exits with `Ok(())`. The output supervisor treats that as terminal and exits without restarting. The output supervisor's inner task is **not** cancelled by the shutdown signal, so buffered messages are not dropped.
3. main joins the output supervisor, then drops its master clone of the stats channel `Sender`. The stats watcher's `recv()` returns `None` and it exits.

The process then returns `0`. There is no shutdown timeout; if you need to force-exit (for example, if the output is stuck mid-reconnect with messages still queued), send a second signal and the runtime will abort.
