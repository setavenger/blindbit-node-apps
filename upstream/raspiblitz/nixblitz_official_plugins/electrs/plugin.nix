{pluginCfg ? {}}: {
  config,
  lib,
  pkgs,
  ...
}: let
  # ── user config ──────────────────────────────────────────────
  address = pluginCfg.address or "127.0.0.1";
  port = pluginCfg.port or 50001;
  dbDir = pluginCfg.db_dir or "/var/lib/electrs";
  openFirewall = pluginCfg.open_firewall or false;

  # ── dashboard tile ───────────────────────────────────────────
  # Polled every 30 s by the TUI's plugin tile machinery. Runs as
  # the admin user — no sudo, no bitcoind RPC access from this
  # context. Reports electrs's *own* state only; the existing
  # Bitcoin tile already shows the chain tip, so "lag" is the
  # operator's eye-job between the two tiles.
  #
  # State machine:
  #   service inactive / failed / dead     → error  ("down")
  #   active + Electrum RPC times out      → warn   ("syncing")
  #     (electrs binds the port but doesn't answer until the
  #     initial index is far enough along; treat that as syncing)
  #   active + RPC returns a height        → ok     ("active")
  tileStateScript = pkgs.writeShellScriptBin "electrs-tile-state" ''
    set -eu

    state=$(${pkgs.systemd}/bin/systemctl is-active electrs 2>/dev/null \
      || echo "inactive")

    case "$state" in
      active) ;;
      activating)
        ${pkgs.jq}/bin/jq -n \
          --arg bind "${address}:${toString port}" \
          '{
            _status_label: "starting",
            _status_color: "warn",
            bind: $bind
          }'
        exit 0
        ;;
      *)
        ${pkgs.jq}/bin/jq -n \
          --arg state "$state" \
          --arg bind "${address}:${toString port}" \
          '{
            _status_label: "down",
            _status_color: "error",
            _footer: ("electrs.service is " + $state),
            _footer_color: "error",
            bind: $bind
          }'
        exit 0
        ;;
    esac

    # Probe the Electrum RPC over bash's built-in /dev/tcp instead
    # of netcat. The OpenBSD-vs-GNU netcat split is a constant
    # source of grief — OpenBSD `nc` has -N but not -q, GNU `nc`
    # the reverse, and which one a given NixOS install resolves
    # `pkgs.netcat-gnu` / `pkgs.netcat` to is not always what you
    # expect. /dev/tcp needs nothing but bash + a Linux kernel.
    #
    # `read -t3` caps the wait at 3 s; the Electrum protocol is
    # newline-delimited JSON-RPC so a single `read` grabs the
    # whole response.
    resp=""
    if exec 3<>"/dev/tcp/${address}/${toString port}" 2>/dev/null; then
      printf '{"id":1,"method":"blockchain.headers.subscribe","params":[]}\n' >&3
      read -t3 resp <&3 || true
      exec 3<&- 3>&-
    fi
    height=$(echo "$resp" | ${pkgs.jq}/bin/jq -r '.result.height // empty' 2>/dev/null \
      || true)

    if [ -z "$height" ]; then
      ${pkgs.jq}/bin/jq -n \
        --arg bind "${address}:${toString port}" \
        '{
          _status_label: "syncing",
          _status_color: "warn",
          _footer: "Electrum RPC not yet answering \u2014 index still building",
          _footer_color: "warn",
          bind: $bind,
          height: "(syncing)"
        }'
      exit 0
    fi

    ${pkgs.jq}/bin/jq -n \
      --arg bind "${address}:${toString port}" \
      --arg height "$height" \
      '{
        _status_label: "active",
        _status_color: "ok",
        bind: $bind,
        height: $height
      }'
  '';

  # On-demand app-version command — plugin.json's `app_version`
  # invokes this by bare name (resolved via PATH thanks to
  # systemPackages). electrs's --version reports the running build,
  # so this stays honest and self-updating.
  appVersionScript = pkgs.writeShellScriptBin "electrs-app-version" ''
    ${pkgs.electrs}/bin/electrs --version
  '';
in {
  # Defensive gate. nix-bitcoin's electrs.nix already asserts
  # `bitcoind.prune == 0` for us — don't duplicate it. We do
  # gate on NixBlitz's own bitcoind feature so the operator gets
  # a clear message if they enabled the plugin without the host
  # service.
  assertions = [
    {
      assertion =
        (config.nixblitz.appConfigs.bitcoind or {}).enabled or false;
      message = "electrs plugin requires bitcoind to be enabled (set app_configs.bitcoind.enabled = true in config.json).";
    }
  ];

  services.electrs = {
    enable = true;
    inherit address port;
    dataDir = dbDir;
  };

  # The upstream module binds Electrum RPC to `cfg.address` only;
  # opening the firewall port lets the LAN reach it when the
  # operator chose a non-loopback bind. For 127.0.0.1 binds the
  # firewall toggle is a no-op, which is fine.
  networking.firewall.allowedTCPPorts =
    lib.mkIf openFirewall [port];

  # Privileged manifest action. Type=oneshot so `systemctl start
  # --wait` in the TUI's plugin runner returns when the restart
  # completes. The bare `systemctl restart electrs` blocks until
  # the new electrs is up (or fails), giving the operator
  # immediate feedback.
  systemd.services.electrs-plugin-restart = {
    description = "Electrs: restart service (plugin action)";
    serviceConfig = {
      Type = "oneshot";
      RemainAfterExit = false;
      # Conservative hardening for a root oneshot: private /tmp, no /home
      # access, no privilege re-escalation, /usr + /boot + /etc read-only.
      # Deliberately NOT ProtectSystem=strict: these units talk to daemons
      # over unix sockets under /run and touch state under /var, and
      # strict's read-only root breaks both. Validate on a node before
      # tightening further.
      PrivateTmp = true;
      ProtectHome = true;
      NoNewPrivileges = true;
      ProtectSystem = "full";
    };
    script = ''
      ${pkgs.systemd}/bin/systemctl restart electrs
    '';
  };

  # Make the dashboard tile-state script discoverable on PATH.
  # The TUI's plugin tile poller invokes it by bare name.
  environment.systemPackages = [tileStateScript appVersionScript];
}
