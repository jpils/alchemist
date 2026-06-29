{
	description = "Rust dev env (NixOS)";

	inputs = {
		nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

		flake-utils.url = "github:numtide/flake-utils";

		fenix = {
			url = "github:nix-community/fenix";
			inputs.nixpkgs.follows = "nixpkgs";
		};

		crane.url = "github:ipetkov/crane";
	};

	outputs = { self, nixpkgs, flake-utils, fenix, crane, ... }:
		flake-utils.lib.eachDefaultSystem (system:
			let
				pkgs = import nixpkgs { inherit system; };

				# Stable Rust toolchain + the usual components
				tc = fenix.packages.${system}.stable;
				toolchain = tc.withComponents [
					"cargo"
					"clippy"
					"rust-src"
					"rustc"
					"rustfmt"
					"rust-analyzer"
				];

				# Optional: make `nix build` work nicely for Rust projects
				craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
				src = craneLib.cleanCargoSource ./.;
				commonArgs = {
					inherit src;
					strictDeps = true;

					# PyO3 needs Python at build time. Runtime UPET deps still come from Pixi.
					nativeBuildInputs = [ pkgs.pkg-config ];
					buildInputs = [ pkgs.python3 ];
					PYO3_PYTHON = "${pkgs.python3}/bin/python3";
				};

				cargoArtifacts = craneLib.buildDepsOnly commonArgs;
				crate = craneLib.buildPackage (commonArgs // { inherit cargoArtifacts; });
			in
				{
				devShells.default = pkgs.mkShell {
					packages = [
						toolchain
						pkgs.bacon
						pkgs.pixi
					];

					# Helps rust-analyzer find std sources
					RUST_SRC_PATH = "${tc.rust-src}/lib/rustlib/src/rust/library";

					shellHook = ''
						PROJECT_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
						export PYO3_PYTHON="$PROJECT_ROOT/.pixi/envs/default/bin/python"
						export PYTHONPATH="$PROJECT_ROOT/python:$PYTHONPATH"

						if [ ! -x "$PYO3_PYTHON" ]; then
							echo "Pixi env missing: $PYO3_PYTHON"
							echo "Run: pixi install"
						else
							echo "PyO3 Python: $PYO3_PYTHON"
						fi
					'';
				};

				packages.default = crate;
			});
}
