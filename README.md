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

Each side (input, output, stats) runs under its own supervisor task. If a side exits with an error it is restarted with exponential backoff (1s → 60s, reset after 60s of stable runtime). A graceful peer disconnect is not treated as an error and does not trigger a restart loop.

### Graceful shutdown

acars-bridge handles `SIGINT` (Ctrl-C) and `SIGTERM` with a coordinated drain:

1. The shutdown signal is broadcast to all tasks.
2. The input task is joined first so no new messages enter the bridge channel.
3. The output task drains any remaining queued messages, then exits.
4. The stats task flushes a final stats line and exits.

The process then returns `0`. There is no shutdown timeout; if you need to force-exit, send a second signal and the runtime will abort.
