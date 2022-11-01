### Builder stage ###
FROM docker.io/library/rust:bullseye AS build-env

WORKDIR /usr/src/app
COPY . .

ENV RUSTFLAGS '-C target-feature=+crt-static'

# Static linking requires to specify a target explicitly
# (see https://github.com/rust-lang/rust/issues/78210).
RUN cargo build \
    --target $(rustup target list | grep -i installed | tr ' ' '\n' | head -1) \
    --release

### Runtime stage ###
FROM scratch

COPY --from=build-env /usr/src/app/target/*/release/http-404 .

EXPOSE 8080/tcp

ENTRYPOINT ["/http-404"]
