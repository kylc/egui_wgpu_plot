{ pkgs ? import <nixpkgs> {} }:

with pkgs;

mkShell {
  nativeBuildInputs = [
    cmake pkg-config fontconfig
    xorg.libX11 xorg.libXcursor xorg.libXrandr xorg.libXi
    vulkan-tools vulkan-loader

    rust-analyzer
  ];

  shellHook = ''
    LD_LIBRARY_PATH="${lib.strings.makeLibraryPath [ xorg.libX11 xorg.libXcursor xorg.libXrandr xorg.libXi vulkan-loader ]}"
  '';
}
