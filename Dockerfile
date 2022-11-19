### Builder stage ###
FROM docker.io/library/rust:bullseye AS build-env

WORKDIR /usr/src/app
COPY . .

ENV CARGO_NET_GIT_FETCH_WITH_CLI true
ENV RUSTFLAGS '-C target-feature=+crt-static'

# Static linking requires to specify a target explicitly
# (see https://github.com/rust-lang/rust/issues/78210).
RUN cargo build \
    --target $(rustup target list | grep -i installed | tr ' ' '\n' | head -1) \
    --release

### Runtime stage ###
FROM scratch

LABEL org.opencontainers.image.source https://github.com/aeron/404-container
LABEL org.opencontainers.image.licenses ISC

COPY --from=build-env /usr/src/app/target/*/release/http-404 .

ENV PORT 8080
EXPOSE ${PORT}/tcp

ENTRYPOINT ["/http-404"]
