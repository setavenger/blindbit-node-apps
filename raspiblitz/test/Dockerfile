# Debian bookworm test bed for bonus.friglet.sh, built by run.sh from a prepared context.
#
# systemd cannot run as PID 1 here: Docker without --privileged mounts the cgroup tree
# read-only and systemd 252 exits with "Failed to create /init.scope control group".
# docker-systemctl-replacement (pinned in run.sh) therefore stands in for systemctl: it
# parses the unit files the script writes and runs them (User=, Environment=,
# EnvironmentFile=, ExecStartPre=, reload, enable/disable). Two differences from systemd
# matter here: it answers unit queries only for root (rootfs/usr/bin/systemctl forwards
# non-root calls through sudo), and it reads EnvironmentFile= once before ExecStartPre
# (systemd reads it per command), so prestart output is only seen from the next start.
FROM debian:bookworm-slim@sha256:3783cc01769c7b2b1b83a5c5ad96c815348e28ed7da68e2e3687004faa906251

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update -qq \
 && apt-get install -y -qq --no-install-recommends \
      sudo nginx-light openssl curl ca-certificates jq whiptail python3 iproute2 \
      adduser procps util-linux bsdutils >/dev/null \
 && apt-get clean && rm -rf /var/lib/apt/lists/*

COPY systemctl3.py /usr/bin/systemctl3.py
COPY rootfs/ /
COPY rb/ /opt/rb/
COPY release/ /srv/release/
COPY scripts/ /opt/test/scripts/
COPY scenarios.sh /opt/test/scenarios.sh

RUN set -e; \
    chmod 755 /usr/bin/systemctl3.py /usr/bin/systemctl /usr/sbin/ufw /usr/local/bin/* /home/admin/_cache.sh /opt/test/scenarios.sh; \
    useradd -M -d /home/admin -s /bin/bash admin; chown admin:admin /home/admin; chmod 755 /home/admin; \
    echo 'admin ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/admin; chmod 440 /etc/sudoers.d/admin; \
    adduser --system --group --home /home/bitcoin bitcoin >/dev/null; \
    adduser --system --group --no-create-home debian-tor >/dev/null; \
    mkdir -p /home/admin/config.scripts /mnt/hdd/app-data/bitcoin /mnt/hdd/app-data/nginx \
             /mnt/hdd/app-data/tor/torrc.d /var/cache/raspiblitz /var/cache/stub /etc/nginx/snippets; \
    chmod 777 /var/cache/stub; echo ready > /var/cache/stub/state; \
    install -m 755 -o admin -g admin /opt/rb/blitz.conf.sh /opt/rb/tor.onion-service.sh /home/admin/config.scripts/; \
    install -m 644 /opt/rb/ssl-params.conf /opt/rb/ssl-certificate-app-data.conf \
            /opt/rb/ssl-certificate-app-data-tor.conf /opt/rb/ssl-proxy-params.conf /etc/nginx/snippets/; \
    touch /mnt/hdd/app-data/tor/torrc.d/services; chown -R bitcoin:bitcoin /mnt/hdd/app-data/tor/torrc.d; \
    openssl req -x509 -newkey rsa:2048 -nodes -days 30 -subj /CN=raspiblitz \
      -keyout /mnt/hdd/app-data/nginx/tls.key -out /mnt/hdd/app-data/nginx/tls.cert 2>/dev/null; \
    openssl dhparam -dsaparam -out /etc/ssl/certs/dhparam.pem 2048 2>/dev/null; \
    touch /var/log/fake-bitcoind.starts; chown bitcoin:bitcoin /var/log/fake-bitcoind.starts; \
    systemctl enable nginx bitcoind tor@default >/dev/null 2>&1 || true

CMD ["python3", "/usr/bin/systemctl3.py"]
