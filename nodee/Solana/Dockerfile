FROM ubuntu:24.04

ARG AGAVE_RELEASE=stable

ENV DEBIAN_FRONTEND=noninteractive
ENV PATH="/home/sol/.local/share/solana/install/active_release/bin:${PATH}"

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        libssl3 \
        libudev1 \
        tini \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --uid 10001 sol \
    && mkdir -p /solana/ledger /solana/accounts /solana/config /solana/logs \
    && chown -R sol:sol /solana /home/sol

USER sol

RUN sh -c "$(curl -sSfL https://release.anza.xyz/${AGAVE_RELEASE}/install)"

USER root

COPY solana-rpc-entrypoint.sh /usr/local/bin/solana-rpc-entrypoint.sh

RUN chmod +x /usr/local/bin/solana-rpc-entrypoint.sh \
    && chown sol:sol /usr/local/bin/solana-rpc-entrypoint.sh

USER sol

EXPOSE 8899/tcp
EXPOSE 8900/tcp
EXPOSE 8000-8029/tcp
EXPOSE 8000-8029/udp

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/solana-rpc-entrypoint.sh"]
