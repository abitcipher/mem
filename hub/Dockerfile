# The MEM hub as one container. Build context: the repository root (the NIF builds
# mem-proto and mem-crypto from crates/). Listens on 4000 inside; put TLS in front of it.

FROM elixir:1.20.2-otp-29 AS build
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates build-essential \
    && rm -rf /var/lib/apt/lists/* \
    && curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.99.0
ENV PATH=/root/.cargo/bin:$PATH MIX_ENV=prod EXQLITE_FORCE_BUILD=true
WORKDIR /src
COPY . .
WORKDIR /src/hub
RUN mix local.hex --force && mix local.rebar --force \
    && mix deps.get --only prod && mix deps.compile && mix release mem_hub

FROM elixir:1.20.2-otp-29-slim
RUN useradd --system --home /var/lib/mem_hub --shell /usr/sbin/nologin memhub \
    && mkdir -p /var/lib/mem_hub && chown memhub /var/lib/mem_hub
COPY --from=build /src/hub/_build/prod/rel/mem_hub /opt/mem_hub
USER memhub
ENV MEM_HUB_PORT=4000 MEM_HUB_DB=/var/lib/mem_hub/mem_hub.db
VOLUME /var/lib/mem_hub
EXPOSE 4000
CMD ["/opt/mem_hub/bin/mem_hub", "start"]
