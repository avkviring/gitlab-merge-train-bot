FROM rust:1.72.0 as builder
WORKDIR /tmp/bot/
COPY . .
RUN apt update
RUN apt install musl-tools -y
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo install --target x86_64-unknown-linux-musl --path .
FROM debian:buster-slim
RUN apt-get update & apt-get install -y extra-runtime-dependencies & rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/gitlab-merge-train-bot /gitlab-merge-train-bot
CMD ["/gitlab-merge-train-bot"]