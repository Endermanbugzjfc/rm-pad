{
  description = "rm-pad prebuilt binary (auto-updated) plus dev shell and source package";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/release-26.05";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        # Pinned release artefact, refreshed daily by the update-bin-lock action.
        binLock = builtins.fromJSON (builtins.readFile ./bin-lock.json);
      in
      {
        packages = {
          # This branch exists to run the prebuilt binary:
          #   nix run github:Endermanbugzjfc/rm-pad/tag-bin
          default = self.packages.${system}.rm-pad-bin;
          rm-pad-bin = pkgs.callPackage ./nix/package-bin.nix {
            inherit (binLock) tag url sha256;
          };
          rm-pad = throw "github:Endermanbugzjfc/rm-pad/tag-bin branch is for the the binary package only; To build from source, use github:alvesvaren/rm-pad";
        };

        devShells.default = pkgs.callPackage ./nix/shell.nix { };
      });
}
