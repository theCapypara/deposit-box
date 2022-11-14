FROM rust:alpine as builder

RUN apk add --no-cache musl-dev openssl openssl-dev pkgconfig glib-dev
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /src
RUN USER=root cargo new --bin deposit-box
WORKDIR /src/deposit-box
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --target x86_64-unknown-linux-musl --release  # collects dependencies
RUN rm src/*.rs  # removes the `cargo new` generated files.

ADD . ./

RUN rm ./target/x86_64-unknown-linux-musl/release/deps/deposit_box*

RUN cargo build --target x86_64-unknown-linux-musl --release

RUN strip /src/deposit-box/target/x86_64-unknown-linux-musl/release/deposit-box

FROM alpine:latest

ARG APP=/usr/src/app

EXPOSE 34434

ENV TZ=Etc/UTC \
    APP_USER=depositbox \
    RUST_LOG="rocket=info,deposit_box=info"

RUN addgroup -S $APP_USER \
    && adduser -S -g $APP_USER $APP_USER

RUN apk update \
    && apk add --no-cache ca-certificates tzdata \
    && rm -rf /var/cache/apk/*

COPY --from=builder /src/deposit-box/target/x86_64-unknown-linux-musl/release/deposit-box ${APP}/deposit-box
COPY view/static ${APP}/view/static
COPY Rocket.toml ${APP}/

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

ENTRYPOINT ["./deposit-box"]
