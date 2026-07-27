{
  lib,
  callPackage,
  rustPlatform,
  pkg-config,
}:
let
  common = callPackage ./common.nix { };
in
rustPlatform.buildRustPackage ({
  pname = "rm-pad";
  version = (lib.importTOML ../Cargo.toml).package.version;

  src = lib.cleanSourceWith {
    src = ../.;
    filter = path: _type:
      let base = baseNameOf path;
      in !(builtins.elem base [ "target" ".direnv" "result" ]);
  };

  cargoLock.lockFile = ../Cargo.lock;

  # build.rs cross-compiles the ARM helper; openssl/display-info need pkg-config.
  nativeBuildInputs = [ pkg-config ] ++ common.crossCompilers;
  buildInputs = common.runtimeLibs;

  meta = {
    description = "Forward reMarkable tablet input to your computer as libinput devices";
    homepage = "https://github.com/alvesvaren/rm-pad";
    license = with lib.licenses; [ mit asl20 ];
    mainProgram = "rm-pad";
    platforms = lib.platforms.linux;
  };
} // common.env)
