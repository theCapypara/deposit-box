FROM rust:slim-buster as builder

RUN apt-get update && apt-get install -y \
  libssl-dev \
  pkg-config \
  libglib2.0-dev \
  && rm -rf /var/lib/apt/lists/* 

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


FROM rust:slim-buster as build

ARG APP=/usr/src/app

EXPOSE 34434

ENV TZ=Etc/UTC \
    APP_USER=depositbox \
    RUST_LOG="rocket=info,deposit_box=info"

RUN adduser --system --group $APP_USER

RUN apt-get update && apt-get install -y \
  ca-certificates \
  tzdata \
  libglib2.0 \
  && rm -rf /var/lib/apt/lists/*


COPY --from=builder /src/deposit-box/target/release/deposit-box ${APP}/deposit-box
COPY view/static ${APP}/view/static
COPY Rocket.toml ${APP}/

RUN chown -R $APP_USER:$APP_USER ${APP}

USER $APP_USER
WORKDIR ${APP}

ENTRYPOINT ["./deposit-box"]
