{
  description = "RustRed development and test environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = { self, nixpkgs }:
    let
      supportedSystems = [ "x86_64-linux" "aarch64-linux" ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in {
      devShells = forAllSystems (system:
        let pkgs = import nixpkgs { inherit system; };
        in {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustc
              cargo
              cargo-nextest
              gcc
              gnum4
              gnumake
              pkg-config
              perl
              git
              cacert
            ];

            # Provide SYMBOLICA_LICENSE in the caller environment.  The
            # repository never embeds a user or CI license value.
            SYMBOLICA_HIDE_BANNER = "1";
          };
        });
    };
}
