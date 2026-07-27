{
  mkShell,
  lib,
  writeShellScriptBin,
  rustc,
  cargo,
  rust-analyzer,
  clippy,
  rustfmt,
  pkg-config,
  openssl,
  libxcb,
  wayland,
  libxkbcommon,
  pkgsCross,
  rustPlatform,
}:
let
  # Cross toolchain for the evdev grab helper (build.rs compiles
  # helper/evgrab.c for the reMarkable's ARM CPUs). The helper links with
  # -static, so wrap each cross gcc to also search the static glibc, whose
  # libc.a lives in a separate output. Using the glibc (not musl) toolchains
  # keeps everything in the binary cache instead of building gcc from source.
  mkCc = crossPkgs: let
    cc = crossPkgs.stdenv.cc;
    staticLibc = crossPkgs.glibc.static;
    name = "${cc.targetPrefix}gcc";
  in
  writeShellScriptBin name ''
    exec ${cc}/bin/${name} -L${staticLibc}/lib "$@"
  '';

  armv7Cc = mkCc pkgsCross.armv7l-hf-multiplatform;
  aarch64Cc = mkCc pkgsCross.aarch64-multiplatform;

  # Native libraries that display-info links against on Linux
  # (xcb for X11, wayland/xkbcommon for Wayland).
  nativeLibs = [
    openssl
    libxcb
    wayland
    libxkbcommon
  ];
in
mkShell {
  nativeBuildInputs = [
    rustc
    cargo
    rust-analyzer
    clippy
    rustfmt
    pkg-config
    armv7Cc
    aarch64Cc
  ];

  buildInputs = nativeLibs;

  # build.rs looks up cross-compilers by name; point it at the wrappers above.
  ARMV7_CC = "${armv7Cc}/bin/armv7l-unknown-linux-gnueabihf-gcc";
  AARCH64_CC = "${aarch64Cc}/bin/aarch64-unknown-linux-gnu-gcc";

  # display-info dlopens some of these at runtime.
  LD_LIBRARY_PATH = lib.makeLibraryPath nativeLibs;

  RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
}
