# acars-bridge

Small utility to bridge from the SDR-E ACARS/HFDL/VDL2 decoder containers to acars_router.

## Usage

acars-bridge can be used to connect to a running SDR-E container and bridge the output to acars_router. It can accept input over UDP, TCP or ZMQ, and optionally output over UDP, TCP or ZMQ. It is a replacement for the TCP/UDP internal wiring that was used in the containers to both send data to acars_router (in the case of `acarsdec`/`vdlm2dec`) and to output stats to container logs.

### Command line flags

| Flag                     | Description                                                                    | Default |
| ------------------------ | ------------------------------------------------------------------------------ | ------- |
| `--log-level`            | Log level to use. `info`, `debug`, and `trace` are valid inputs.               | `info`  |
| `--source-host`          | Hostname or IP address where the decoder is getting data from.                 | `unset` |
| `--source-port`          | Port where the decoder is getting data from.                                   | `unset` |
| `--source-protocol`      | Protocol to use for the source. `udp`, `tcp`, and `zmq` are valid inputs.      | `unset` |
| `--destination-host`     | Hostname or IP address where acars_router is running.                          | `unset` |
| `--destination-port`     | Port where acars_router is running.                                            | `unset` |
| `--destination-protocol` | Protocol to use for the destination. `udp`, `tcp`, and `zmq` are valid inputs. | `unset` |
| `--stats-interval`       | Interval in minutes to output stats to the log.                                | `5`     |
