{ pkgs ? import <nixpkgs> {} }:

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

  shellHook = ''
    export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath (with pkgs; [
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
    ])}:$LD_LIBRARY_PATH"
    echo "🎵 Welcome to tntron (Rust + iced GUI) development environment!"
  '';
}
