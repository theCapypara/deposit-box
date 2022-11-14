FROM rust:alpine as builder

RUN apk add --no-cache musl-dev openssl openssl-dev pkgconfig glib-dev

WORKDIR /src
RUN USER=root cargo new --bin deposit-box
WORKDIR /src/deposit-box
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN cargo build --release  # collects dependencies
RUN rm src/*.rs  # removes the `cargo new` generated files.

ADD . ./

RUN rm ./target/release/deps/deposit_box*

RUN cargo build --release

RUN strip /src/deposit-box/target/release/deposit-box

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

COPY --from=builder /src/deposit-box/target/release/deposit-box ${APP}/deposit-box
COPY view/static view/static

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

ENTRYPOINT ["./deposit-box"]
