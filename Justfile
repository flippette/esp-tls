rr3:
  cargo run --release \
    --target riscv32imc-unknown-none-elf \
    --features esp32c3

rr6:
  ESP_HAL_CONFIG_FLIP_LINK=true \
    cargo run --release \
      --target riscv32imac-unknown-none-elf \
      --feature esp32c6

br3:
  cargo build --release \
    --target riscv32imc-unknown-none-elf \
    --features esp32c3

br6:
  ESP_HAL_CONFIG_FLIP_LINK=true \
    cargo build --release \
      --target riscv32imac-unknown-none-elf \
      --features esp32c6

cl:
  cargo clean
