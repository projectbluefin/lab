{ lib
, rustPlatform
}:

let
  cargoToml = builtins.fromTOML (builtins.readFile ../Cargo.toml);
in
rustPlatform.buildRustPackage {
  pname = cargoToml.package.name;
  inherit (cargoToml.package) version;

  src = lib.sourceFilesBySuffices ./.. [ ".rs" ".toml" ".lock" ];

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  meta = with lib; {
    homepage = "https://github.com/nikstur/uapi-version";
    license = licenses.mit;
    maintainers = with lib.maintainers; [ nikstur ];
  };
}
