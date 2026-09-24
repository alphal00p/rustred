{
  description = "RustRed development and test environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in {
      apps = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          pythonExamples = ./examples/python;
          pythonApp = name: script: {
            type = "app";
            program = "${pkgs.writeShellApplication {
              inherit name;
              runtimeInputs = [ pkgs.python311 ];
              text = ''exec python ${pythonExamples}/${script} "$@"'';
            }}/bin/${name}";
          };
        in {
          campaign = pythonApp "rustred-campaign" "shared_owner_campaign.py";
          campaign-monitor = pythonApp "rustred-campaign-monitor" "campaign_monitor.py";
          campaign-production = pythonApp "rustred-campaign-production" "production_saved_owner_campaign.py";
          campaign-stage = pythonApp "rustred-campaign-stage" "stage_saved_owner_campaign.py";
        });
      devShells = forAllSystems (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustc
              cargo
              rustfmt
              cargo-nextest
              maturin
              uv
              binutils
              (python311.withPackages (pythonPackages: with pythonPackages; [
                auditwheel
                pip
                virtualenv
              ]))
              gcc
              gnum4
              gnumake
              pkg-config
              perl
              git
              cacert
              procps
              util-linux
            ];

            # Provide SYMBOLICA_LICENSE in the caller environment.  The
            # repository never embeds a user or CI license value.
            SYMBOLICA_HIDE_BANNER = "1";
          };
        });
    };
}
