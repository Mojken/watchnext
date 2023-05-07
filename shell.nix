{ pkgs ? import <nixpkgs> {} }:

with pkgs;

let
  python-requirements = ps: with ps; [
    python-vlc
    (
      buildPythonPackage rec {
        pname = "mpris_server";
        version = "0.4.3";

        src = fetchPypi {
          inherit pname version;
          hash = "sha256-CjITYptnKuj/z0Y3IakyoivwdJ4+09QoJfGoUtuVwZE=";
        };

        doCheck = false;
        propagatedBuildInputs = [
          (
            buildPythonPackage rec {
              pname = "emoji";
              version = "1.2.0";
              format = "setuptools";

              disabled = pythonOlder "3.7";

              src = fetchFromGitHub {
                owner = "carpedm20";
                repo = pname;
                rev = "refs/tags/v.${version}";
                hash = "sha256-Qinr9gr+Xd6l+wwksQLq7+qZn8+nhXaDehPkCPEakmM=";
              };

              checkInputs = [
                pytestCheckHook
              ];

              disabledTests = [
                "test_emojize_name_only"
              ];

              pythonImportsCheck = [
                "emoji"
              ];

              meta = with lib; {
                description = "Emoji for Python";
                homepage = "https://github.com/carpedm20/emoji/";
                license = licenses.bsd3;
                maintainers = with maintainers; [ joachifm ];
              };
            }
          )
          (
            buildPythonPackage rec {
              pname = "unidecode";
              version = "1.2.0";
              format = "setuptools";

              disabled = pythonOlder "3.5";

              src = fetchFromGitHub {
                owner = "avian2";
                repo = pname;
                rev = "${pname}-${version}";
                hash = "sha256-G8mZd5Y+EYxHoKaW0WiGKtt4sUyVyjOVy+WoF7TjEmc=";
              };

              checkInputs = [
                pytestCheckHook
              ];

              pythonImportsCheck = [
                "unidecode"
              ];

              meta = with lib; {
                description = "ASCII transliterations of Unicode text";
                homepage = "https://pypi.python.org/pypi/Unidecode/";
                license = licenses.gpl2Plus;
                maintainers = with maintainers; [ domenkozar ];
              };
            }
          )
          pygobject3
          pydbus
        ];

        meta = with lib; {
          homepage = "https://github.com/alexdelorenzo/mpris_server";
          description = "Integrate MPRIS Media Player support into your app ";
          license = "licenses.agpl3";
          maintainers = with maintainers; [ alexdelorenzo ];
        };
      }
    )
  ];
in
mkShell {
  buildInputs = [
    htop
    vlc
    (python3.withPackages python-requirements)
    git
  ];
}
