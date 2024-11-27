{
  outputs =
    {
      self,
    }:
    {
      nixosModules.default =
        {
          pkgs,
          lib,
          config,
          ...
        }:
        let
          cfg = config.services.packwiz;
          toml = pkgs.formats.toml { };
          configFilePath = toml.generate "config.toml" cfg.configuration;
        in
        {
          options.services.packwiz = {
            enable = lib.mkEnableOption "enable server to serve packwiz modpack";
            port = lib.mkOption {
              default = 8787;
              type = lib.types.int;
            };
            listen = lib.mkOption {
              default = "[::]:${toString cfg.port}";
              type = lib.types.str;
              description = ''
                The "ListenStream" used in static-web-server.socket.
                This is equivalent to SWS's "host" and "port" options.
                See here for specific syntax: <https://www.freedesktop.org/software/systemd/man/systemd.socket.html#ListenStream=>
              '';
            };
            root = lib.mkOption {
              type = lib.types.path;
              default = ./.;
              description = ''
                The location of files for SWS to serve. Equivalent to SWS's "root" config value.
                NOTE: This folder must exist before starting SWS.
              '';
            };
            configuration = lib.mkOption {
              default = { };
              type = toml.type;
              example = {
                general = {
                  log-level = "error";
                  directory-listing = true;
                };
              };
              description = ''
                Configuration for Static Web Server. See
                <https://static-web-server.net/configuration/config-file/>.
                NOTE: Don't set "host", "port", or "root" here. They will be ignored.
                Use the top-level "listen" and "root" options instead.
              '';
            };
          };

          config = lib.mkIf cfg.enable {
            environment.systemPackages = [ pkgs.static-web-server ];
            systemd.packages = [ pkgs.static-web-server ];
            # Have to set wantedBy since systemd.packages ignores the "Install" section
            systemd.sockets.packwiz = {
              wantedBy = [ "sockets.target" ];
              # Start with empty string to reset upstream option
              listenStreams = [
                ""
                cfg.listen
              ];
            };
            systemd.services.packwiz = {
              wantedBy = [ "multi-user.target" ];
              serviceConfig = {
                # Remove upstream sample environment file; use config.toml exclusively
                EnvironmentFile = [ "" ];
                ExecStart = [
                  ""
                  "${pkgs.static-web-server}/bin/static-web-server --fd 0 --config-file ${configFilePath} --root ${cfg.root}"
                ];
                # Supplementary groups doesn't work unless we create the group ourselves
                SupplementaryGroups = [ "" ];
                # If the user is serving files from their home dir, override ProtectHome to allow that
                ProtectHome = if lib.hasPrefix "/home" cfg.root then "tmpfs" else "true";
                BindReadOnlyPaths = cfg.root;
              };
            };
          };
        };
    };
}