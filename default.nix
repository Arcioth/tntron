{
  lib,
  rustPlatform,
  pkg-config,
  makeWrapper,
  mpv,
  libjxl,
  imagemagick,
  yt-dlp,
  wayland,
  libxkbcommon,
  vulkan-loader,
  libGL,
  fontconfig,
  freetype,
  libx11,
  libxcursor,
  libxi,
  libxrandr,
}:

let
  runtimeLibs = [
    wayland
    libxkbcommon
    vulkan-loader
    libGL
    fontconfig
    freetype
    libx11
    libxcursor
    libxi
    libxrandr
  ];

  runtimeBins = [
    mpv
    libjxl
    imagemagick
    yt-dlp
  ];
in
rustPlatform.buildRustPackage {
  pname = "tntron";
  version = "0.1.0";

  src = lib.cleanSourceWith {
    src = ./.;
    filter =
      path: type:
      let
        rel = lib.removePrefix (toString ./. + "/") (toString path);
      in
      !(lib.hasPrefix "target" rel || lib.hasPrefix "dist" rel || lib.hasPrefix "result" rel);
  };

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = [
    pkg-config
    makeWrapper
  ];

  buildInputs = [
    mpv
  ] ++ runtimeLibs;

  # Tests need a writable HOME directory inside the Nix sandbox.
  preCheck = ''
    export HOME=$(mktemp -d)
    export XDG_CONFIG_HOME="$HOME/.config"
    export XDG_DATA_HOME="$HOME/.local/share"
    export XDG_RUNTIME_DIR=$(mktemp -d)
  '';

  postInstall = ''
    # Desktop entry and hicolor icons (XFCE, GNOME, KDE, Hyprland, Sway, etc.)
    install -Dm644 assets/tntron.desktop "$out/share/applications/tntron.desktop"
    install -Dm644 assets/icons/tntron-logo.svg "$out/share/tntron/tntron-logo.svg"
    install -Dm644 assets/icons/hicolor/scalable/apps/tntron.svg \
      "$out/share/icons/hicolor/scalable/apps/tntron.svg"

    for sz in 16 24 32 48 64 128 256 512; do
      install -Dm644 "assets/icons/hicolor/''${sz}x''${sz}/apps/tntron.png" \
        "$out/share/icons/hicolor/''${sz}x''${sz}/apps/tntron.png"
    done

    # Bundled extensions & optional GIF visualizer pack
    mkdir -p "$out/share/tntron/extensions"
    cp -r extensions/. "$out/share/tntron/extensions/"
    if [ -d assets/gifs ]; then
      mkdir -p "$out/share/tntron/gifs"
      cp -r assets/gifs/. "$out/share/tntron/gifs/"
    fi

    # Documentation
    install -Dm644 LICENSE "$out/share/licenses/tntron/LICENSE"
    install -Dm644 README.md "$out/share/doc/tntron/README.md"
    if [ -d docs ]; then
      mkdir -p "$out/share/doc/tntron/docs"
      cp -r docs/. "$out/share/doc/tntron/docs/"
    fi

    # Wrap GUI & CLI binaries with audio, Wayland/X11/GPU, JXL/SVG, and PATH runtime deps
    for bin in tntron tntron-ctl; do
      if [ -f "$out/bin/$bin" ]; then
        wrapProgram "$out/bin/$bin" \
          --prefix PATH : "${lib.makeBinPath runtimeBins}" \
          --prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath runtimeLibs}"
      fi
    done
  '';

  meta = with lib; {
    description = "GPU-native music player workstation powered by Rust, iced, mpv, and sandboxed Lua extensions";
    homepage = "https://github.com/Arcioth/tntron";
    license = licenses.mit;
    mainProgram = "tntron";
    platforms = platforms.linux;
  };
}
