{
  description = "loggen-rs - a log generator";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, naersk, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages."${system}";
        naersk-lib = naersk.lib."${system}";

        loggen = naersk-lib.buildPackage {
          pname = "loggen";
          root = ./.;
        };

        loggen-data = pkgs.stdenv.mkDerivation {
          pname = "loggen-data";
          version = "0.4.1";
          src = ./.;
          installPhase = ''
            mkdir -p $out/usr/share/loggen
            cp -r templates $out/usr/share/loggen/templates
            cp -r examples $out/usr/share/loggen/examples
            cp -r docs $out/usr/share/loggen/docs
          '';
          dontBuild = true;
        };

        container = pkgs.dockerTools.buildImage {
          name = "loggen";
          tag = "latest";
          created = "now";
          copyToRoot = pkgs.buildEnv {
            name = "image-root";
            paths = [ loggen pkgs.busybox pkgs.cacert loggen-data ];
            pathsToLink = [ "/bin" "/usr/share/loggen" ];
          };
          config = {
            Entrypoint = [ "${loggen}/bin/loggen" ];
            WorkingDir = "/usr/share/loggen";
          };
        };
      in
      {
        packages = {
          default = loggen;
          container = container;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [ rustc cargo clippy rustfmt ];
        };
      }
    );
}
