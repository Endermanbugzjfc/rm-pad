{
  lib,
  stdenv,
  fetchurl,
  autoPatchelfHook,
  callPackage,
  # Release tag to fetch (e.g. "v0.2.3"), supplied by the flake as the closest
  # git tag reachable from HEAD.
  tag,
}:
let
  common = callPackage ./common.nix { };

  # The release workflow only publishes an x86_64 Linux tarball.
  assetName = "rm-pad-linux-x86_64.tar.gz";
  tarballUrl = "https://github.com/alvesvaren/rm-pad/releases/download/${tag}/${assetName}";
  checksumUrl = "${tarballUrl}.sha256";

  # Impurely fetch the published checksum (needs `--impure`). This doubles as an
  # existence check: if the tag ships no binary, the checksum 404s and we fail
  # explicitly instead of downloading a GitHub error page. Having the real hash
  # lets the tarball itself be a reproducible fixed-output download.
  checksum = builtins.tryEval (builtins.readFile (builtins.fetchurl checksumUrl));
  sha256 =
    if checksum.success
    then lib.head (lib.splitString " " checksum.value)
    else
      throw ''
        rm-pad-bin: release tag '${tag}' has no ${assetName}.
        Could not fetch ${checksumUrl} — this tag does not ship a prebuilt binary.
      '';
in
assert lib.assertMsg (stdenv.hostPlatform.system == "x86_64-linux") ''
  rm-pad-bin: prebuilt binaries are only published for x86_64-linux (got ${stdenv.hostPlatform.system}).
  Use the `rm-pad` package to build from source instead.'';
stdenv.mkDerivation {
  pname = "rm-pad-bin";
  version = lib.removePrefix "v" tag;

  src = fetchurl {
    url = tarballUrl;
    inherit sha256;
  };

  nativeBuildInputs = [ autoPatchelfHook ];
  buildInputs = common.runtimeLibs ++ [ stdenv.cc.cc.lib ];
  # display-info dlopens some of these; force them onto the rpath.
  runtimeDependencies = common.runtimeLibs;

  # The tarball is a flat archive (./rm-pad, ./data, ./README.md, ...).
  sourceRoot = ".";

  installPhase = ''
    runHook preInstall
    install -Dm755 rm-pad "$out/bin/rm-pad"
    install -Dm644 data/50-uinput.rules "$out/lib/udev/rules.d/50-uinput.rules"
    install -Dm644 data/rm-pad.service "$out/lib/systemd/user/rm-pad.service"
    install -Dm644 rm-pad.toml.example "$out/share/rm-pad/rm-pad.toml.example"
    install -Dm644 README.md "$out/share/doc/rm-pad/README.md"
    runHook postInstall
  '';

  meta = {
    description = "Forward reMarkable tablet input to your computer (prebuilt release binary)";
    homepage = "https://github.com/alvesvaren/rm-pad";
    license = with lib.licenses; [ mit asl20 ];
    mainProgram = "rm-pad";
    platforms = [ "x86_64-linux" ];
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
  };
}
