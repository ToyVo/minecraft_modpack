{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    devshell = {
      url = "github:numtide/devshell";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay.url = "github:oxalica/rust-overlay";
    nixpkgs-dioxus.url = "github:CathalMullan/nixpkgs/dioxus-cli-v0.6.2";
  };

  nixConfig = {
    extra-substituters = [
      "https://cache.nixos.org"
      "https://nix-community.cachix.org"
      "https://toyvo.cachix.org"
    ];
    extra-trusted-public-keys = [
      "cache.nixos.org-1:6NCHdD59X431o0gWypbMrAURkbJ16ZPMQFGspcDShjY="
      "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
      "toyvo.cachix.org-1:s++CG1te6YaS9mjICre0Ybbya2o/S9fZIyDNGiD4UXs="
    ];
    allow-import-from-derivation = true;
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      flake-parts,
      devshell,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      imports = [
        devshell.flakeModule
        flake-parts.flakeModules.easyOverlay
      ];

      perSystem =
        {
          self',
          system,
          pkgs,
          lib,
          config,
          ...
        }:
        {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [
              inputs.rust-overlay.overlays.default
              (final: prev: {
                dioxus-cli = inputs.nixpkgs-dioxus.legacyPackages.${prev.system}.dioxus-cli;
              })
            ];
          };
          formatter = pkgs.nixfmt-rfc-style;

          packages =
            let
              rev = toString (self.shortRev or self.dirtyShortRev or self.lastModified or "unknown");
            in
            rec {
              rustToolchain = (
                pkgs.rust-bin.stable.latest.default.override {
                  extensions = [
                    "rust-src"
                    "rust-analyzer"
                    "clippy"
                  ];
                  targets = [ "wasm32-unknown-unknown" ];
                }
              );
              cache_modpack =
                let
                  cargoToml = builtins.fromTOML (builtins.readFile ./cache_modpack/Cargo.toml);
                in
                pkgs.rustPlatform.buildRustPackage {
                  pname = cargoToml.package.name;
                  version = "${cargoToml.package.version}-${rev}";
                  src = ./.;
                  strictDeps = true;
                  nativeBuildInputs = with pkgs; [
                    rustToolchain
                    openssl
                    libiconv
                    pkg-config
                    rustPlatform.bindgenHook
                  ];
                  buildInputs =
                    with pkgs;
                    [
                      openssl
                      libiconv
                      pkg-config
                    ]
                    ++ lib.optionals pkgs.stdenv.isDarwin [
                      pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
                    ];
                  cargoLock.lockFile = ./Cargo.lock;
                  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
                };
              minecraft_modpack =
                let
                  cargoToml = builtins.fromTOML (builtins.readFile ./minecraft_modpack/Cargo.toml);
                in
                pkgs.rustPlatform.buildRustPackage {
                  pname = cargoToml.package.name;
                  version = "${cargoToml.package.version}-${rev}";
                  src = ./.;
                  strictDeps = true;
                  nativeBuildInputs = with pkgs; [
                    dioxus-cli
                    rustToolchain
                    openssl
                    libiconv
                    pkg-config
                    wasm-bindgen-cli
                    rustPlatform.bindgenHook
                    cache_modpack
                  ];
                  buildInputs =
                    with pkgs;
                    [
                      openssl
                      libiconv
                      pkg-config
                    ]
                    ++ lib.optionals pkgs.stdenv.isDarwin [
                      pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
                    ];
                  buildPhase = ''
                    cache_modpack -i modpack -o minecraft_modpack/assets/modpack.json
                    dx build --package minecraft_modpack --release --platform web --verbose --trace
                  '';
                  installPhase = ''
                    mkdir -p $out
                    cp -r target/dx/$pname/release/web/public/* $out
                    cp $src/CNAME $out
                    cp -r $src/modpack/* $out
                    cp $out/index.html $out/404.html
                  '';
                  cargoLock.lockFile = ./Cargo.lock;
                };
              default = minecraft_modpack;
            };
          devshells.default = {
            imports = [
              "${devshell}/extra/language/c.nix"
              # "${devshell}/extra/language/rust.nix"
            ];

            env = [
              {
                name = "RUST_LOG";
                value = "minecraft_modded=trace";
              }
              {
                name = "RUST_SRC_PATH";
                value = "${pkgs.rustPlatform.rustLibSrc}";
              }
            ];

            commands = [
              {
                package = pkgs.dioxus-cli;
              }
            ];

            packages =
              with pkgs;
              [
                dioxus-cli
                self'.packages.rustToolchain
                pkg-config
                rustPlatform.bindgenHook
                cargo-watch
                systemfd
              ]
              ++ lib.optionals stdenv.isDarwin [ darwin.apple_sdk.frameworks.SystemConfiguration ];

            language.c = {
              libraries = lib.optional pkgs.stdenv.isDarwin pkgs.libiconv;
            };
          };
        };
    };
}
