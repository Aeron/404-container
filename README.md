# 404 Container

It’s a super-compact container with the sole purpose of responding with HTTP 404.
And it’s written in Rust and `async-std`.

## Motivation

It mainly exists because I wanted a tiny-tiny performant default back-end for my
HAProxy Ingress Kubernetes deployments.

The usual default `k8s.gcr.io/defaultbackend-amd64` does not fit the bill quite well
to my taste. Especially when its `arm64` variant gives you a platform mismatch error,
declaring the image’s platform is `linux/amd64`.

(At least, it has been doing it at the moment of writing this. Who knows why—it’s
four years old.)

Also, there may be a room or a requirement for customization or extra configurability.

## Usage

The container image is available as [`docker.io/aeron/404`][docker] and
[`ghcr.io/Aeron/404`][github]. You can use both interchangeably.

```sh
docker pull docker.io/aeron/404
# …or…
docker pull ghcr.io/aeron/404
```

[docker]: https://hub.docker.com/r/aeron/404
[github]: https://github.com/Aeron/404-container/pkgs/container/404

### Container Running

Running a container is pretty straightforward:

```sh
docker -d --restart unless-stopped --name http-404 \
    --user=65534 \
    -p 80/8080:tcp \
    docker.io/aeron/404
```

By default, the containerized app listens on the `0.0.0.0:8080` address.

Don’t forget about the unprivileged user trick. The container itself won’t enforce
any specific UID.
