{ pkgs ? import <nixpkgs> {} }:

let
  runtimeLibs = with pkgs; [
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
in
pkgs.mkShell {
  name = "tntron-dev";

  nativeBuildInputs = with pkgs; [
    rustc
    cargo
    rustfmt
    clippy
    pkg-config
  ];

  buildInputs = with pkgs; [
    mpv
    ffmpeg-headless
    libjxl
    imagemagick
    yt-dlp
  ] ++ runtimeLibs;

  shellHook = ''
    export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath runtimeLibs}:$LD_LIBRARY_PATH"
    echo "tntron dev environment ready (Rust + iced + mpv + djxl + imagemagick)"
  '';
}
