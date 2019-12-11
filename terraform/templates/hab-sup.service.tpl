[Unit]
Description=Habitat Supervisor

[Service]
# Note: The RUST_LOG line below is meant only for the services
# Currently, the launcher also uses this but that will not always be the case
# Related issue: https://github.com/habitat-sh/habitat/issues/6632
Environment=RUST_LOG=${log_level}
Environment=HAB_STATS_ADDR=localhost:8125
%{ for feature in enabled_features ~}
Environment=HAB_FEAT_${upper(feature)}=1
%{ endfor ~}
ExecStartPre=/bin/bash -c "/bin/systemctl set-environment SSL_CERT_FILE=$(hab pkg path core/cacerts)/ssl/cert.pem"
ExecStart=/bin/hab run ${flags}
ExecStop=/bin/hab sup term
KillMode=process
LimitNOFILE=65535
LimitCORE=infinity

[Install]
WantedBy=default.target
