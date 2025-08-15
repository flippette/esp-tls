# esp-tls

HTTPS demo on ESP32-C3/C6 with bare-metal, async Rust.

this demo was based on my
[`esp-template`](https://github.com/flippette/esp-template).

## running

you'll need a nightly Rust toolchain, and
[`espflash`](https://github.com/esp-rs/espflash).

connect to your board over either UART or JTAG. then,

- to run on ESP32-C3:

```sh
export WIFI_SSID="your wifi ssid"
export WIFI_PASS="your wifi password"
cargo run --release \
  --target riscv32imc-unknown-none-elf \
  --features esp32c3
```

- to run on ESP32-C6:

```sh
export WIFI_SSID="your wifi ssid"
export WIFI_PASS="your wifi password"
cargo run --release \
  --target riscv32imac-unknown-none-elf \
  --features esp32c6
```

this demo expects an open network if `WIFI_PASS` is not set, or a WPA2 Personal
network if it is. `WIFI_SSID` _must_ be set, though.

this demo performs a `GET` request to `https://check.tls.support`, which
returns a JSON document detailing current TLS support status of `esp-mbedtls`.
