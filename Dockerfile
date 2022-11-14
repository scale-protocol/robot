FROM rust:latest AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev git openssh-client
RUN update-ca-certificates

RUN git config --global url."git@github.com:".insteadOf "https://github.com/"
RUN mkdir -p -m 0600 ~/.ssh && ssh-keyscan github.com >> ~/.ssh/known_hosts

WORKDIR /app

ENV CARGO_NET_GIT_FETCH_WITH_CLI=true


COPY ./ .

RUN --mount=type=ssh cargo build --target x86_64-unknown-linux-musl --release

FROM alpine:3.16
WORKDIR /app

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/scale /usr/local/bin/scale
WORKDIR /app
EXPOSE 3000
CMD scale