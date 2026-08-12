{
  description = "nexus-gql workspace: GraphQL client + batch downloader CLI (dev shell only)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          # CI does not pin a toolchain; GitHub runners ship an up-to-date
          # stable, so mirror that with the latest stable release.
          rustToolchain = pkgs.rust-bin.stable.latest.default;
        in
        {
          default = pkgs.mkShell {
            packages = [
              rustToolchain
              # 7zz (nixpkgs `_7zz`); the CLI falls back from `7z` to `7zz`.
              pkgs._7zz
            ];
          };
        }
      );
    };
}
