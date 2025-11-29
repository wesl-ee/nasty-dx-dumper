{
  description = "nasty-dx-dumper";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        isDarwin = pkgs.stdenv.isDarwin;
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "hooya";
          version = "0.1.2";
          src = ./.;

          doCheck = false;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
            protobuf
          ];
        };

        devShells.default = with pkgs; mkShell {
          buildInputs = [
            (rust-bin.stable."1.88.0".default.override {
              extensions = [ "rust-src" ];
            })
            git
            just
          ];
          RUST_SRC_PATH="${pkgs.rust-bin.stable."1.88.0".default}/lib/rustlib/src/rust/library";
        };

        devShells.py = pkgs.mkShell {
          packages = [
            pkgs.python312
            pkgs.python312Packages.venvShellHook
            pkgs.openssl
          ];

          venvDir = ".venv";

  postShellHook = ''
    # optional but handy
    python -m pip install --upgrade pip wheel setuptools

    if [ -f requirements.txt ]; then
      pip install -r requirements.txt
    fi

    # optional: load OPENAI_API_KEY, etc.
    if [ -f .env ]; then set -a; . ./.env; set +a; fi
  '';
        };
      }
    );
}

