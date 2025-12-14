{
  description = "Consumer repo using shared base + rust precommit system";

  inputs = {
    precommit.url = "github:FredSystems/pre-commit-checks";
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs =
    {
      self,
      precommit,
      nixpkgs,
      ...
    }:
    let
      systems = precommit.lib.supportedSystems;
    in
    {
      ##########################################################################
      ## CHECKS — unified base+rust via mkCheck
      ##########################################################################
      checks = builtins.listToAttrs (
        map (system: {
          name = system;
          value = {
            pre-commit-check = precommit.lib.mkCheck {
              src = ./.;

              inherit system;
              check_rust = true;
              check_docker = true;

              extraExcludes = [
                "^speed_tests/"
                "^Documents/"
                "^res/"
                "typos.toml"
              ];
            };
          };
        }) systems
      );

      ##########################################################################
      ## DEV SHELLS — merged env + your extra Rust goodies
      ##########################################################################
      devShells = builtins.listToAttrs (
        map (system: {
          name = system;

          value =
            let
              pkgs = import nixpkgs { inherit system; };

              # Unified check result (base + rust)
              chk = self.checks.${system}."pre-commit-check";

              # Packages that git-hooks.nix / mkCheck say we need
              corePkgs = chk.enabledPackages or [ ];

              # Extra Rust / tooling packages (NO extra rustc here)
              extraRustTools = [
              ];
              # ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
              # ];

              # Extra dev packages provided by mkCheck (includes rustToolchain)
              extraDev = chk.passthru.devPackages or [ ];

              # Library path packages: whatever mkCheck wants + your GL/Wayland bits
              libPkgs = [ ];
              # (chk.passthru.libPath or [ ])
              # ++ [
              # ]
              # ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
              # ];
            in
            {
              default = pkgs.mkShell {
                buildInputs = extraDev ++ corePkgs ++ extraRustTools;

                LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libPkgs;

                shellHook = ''
                  ${chk.shellHook}

                  alias pre-commit="pre-commit run --all-files"
                '';
              };
            };
        }) systems
      );
    };
}
