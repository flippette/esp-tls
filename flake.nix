{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustBin = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustBin;

        # include the PEM certificate as well
        unfilteredRoot = ./.;
        src = with pkgs;
          lib.fileset.toSource {
            root = unfilteredRoot;
            fileset = lib.fileset.unions [
              (craneLib.fileset.commonCargoSources unfilteredRoot)
              ./check.tls.support.chain.pem
            ];
          };

        # common args between packages
        commonArgs = rec {
          inherit src;
          strictDeps = true;

          cargoVendorDir = craneLib.vendorMultipleCargoDeps {
            inherit (craneLib.findCargoFiles src) cargoConfigs;
            cargoLockList = [
              ./Cargo.lock

              # needed for `-Z build-std`
              # <https://crane.dev/examples/build-std.html>
              ("${rustBin.passthru.availableComponents.rust-src}"
                + "/lib/rustlib/src/rust/library/Cargo.lock")
            ];
          };

          doCheck = false;
        };
      in with pkgs; {
        devShells.default = mkShell {
          buildInputs = [ cargo-binutils cargo-bloat espflash rustBin ];
        };

        packages = rec {
          default = esp32c6;

          esp32c3 = craneLib.buildPackage (commonArgs // {
            cargoExtraArgs = pkgs.lib.concatStringsSep " " [
              "--target riscv32imc-unknown-none-elf"
              "--features esp32c3"
            ];
          });

          esp32c6 = craneLib.buildPackage (commonArgs // {
            cargoExtraArgs = pkgs.lib.concatStringsSep " " [
              "--target riscv32imac-unknown-none-elf"
              "--features esp32c6"
            ];

            ESP_HAL_CONFIG_FLIP_LINK = "true";
          });
        };
      });
}
