{ pkgs ? import <nixpkgs> {} }:

with pkgs;

let
  python-requirements = ps: with ps; [
    pip
    pycairo
    python-vlc
  ];
in
mkShell {
  buildInputs = [
    htop
    pkg-config
    cairo
    vlc
    gobject-introspection
    (python3.withPackages python-requirements)
    git
  ];
  shellHook =
    ''
      source .venv/bin/activate
    '';
}
