{
  description = "rm-pad development environment and packages";

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

        # Closest git tag reachable from HEAD, e.g. "v0.2.3". This is inherently
        # impure — flakes don't expose git tags — so building rm-pad-bin needs
        # `nix build .#rm-pad-bin --impure`, run from the repository root. Kept
        # lazy so the rest of the flake stays pure.
        closestTag =
          let
            repoRoot = builtins.getEnv "PWD";
          in
          if repoRoot == "" then
            throw "rm-pad-bin needs an impure build from the repo root: nix build .#rm-pad-bin --impure"
          else
            let
              dotGit = builtins.path {
                path = "${repoRoot}/.git";
                name = "rm-pad-dotgit";
              };
              tagFile = pkgs.runCommandLocal "rm-pad-closest-tag"
                { nativeBuildInputs = [ pkgs.git ]; }
                ''
                  git --git-dir=${dotGit} describe --tags --abbrev=0 | tr -d '\n' > "$out"
                '';
            in
            pkgs.lib.fileContents tagFile;
      in
      {
        packages = {
          default = self.packages.${system}.rm-pad;
          rm-pad = pkgs.callPackage ./nix/package.nix { };
          rm-pad-bin = pkgs.callPackage ./nix/package-bin.nix { tag = closestTag; };
        };

        devShells.default = pkgs.callPackage ./nix/shell.nix { };
      });
}
