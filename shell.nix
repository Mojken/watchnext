{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    man
    btop
    vlc
    git
    rustup
    gdb
    pkg-config
    openssl
  ];
}
