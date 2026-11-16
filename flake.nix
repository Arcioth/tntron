{
  description = "tntron — GPU-native music workstation (Rust + iced + mpv + Lua 5.4 extensions)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      supportedSystems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
      mkPkgs =
        system:
        import nixpkgs {
          inherit system;
          overlays = [ self.overlays.default ];
        };
    in
    {
      overlays.default = final: _prev: {
        tntron = final.callPackage ./default.nix { };
      };

      packages = forAllSystems (
        system:
        let
          pkgs = mkPkgs system;
        in
        {
          default = pkgs.tntron;
          tntron = pkgs.tntron;
        }
      );

      apps = forAllSystems (
        system:
        let
          pkg = self.packages.${system}.tntron;
        in
        {
          default = {
            type = "app";
            program = "${pkg}/bin/tntron";
          };
          tntron = {
            type = "app";
            program = "${pkg}/bin/tntron";
          };
          tntron-ctl = {
            type = "app";
            program = "${pkg}/bin/tntron-ctl";
          };
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
        in
        {
          default = import ./shell.nix { inherit pkgs; };
        }
      );
    };
}
