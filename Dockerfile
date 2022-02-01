FROM rust:1.58 AS builder

# Create appuser
ENV USER=zeroed-books
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /usr/src/zeroed-books-api

# Create a dummy project and run a build with all our dependencies. As long as
# the dependencies don't change, any future builds will only have to recompile
# our code (and not the dependencies).
RUN cargo init
COPY Cargo.toml Cargo.lock ./
RUN cargo build --locked --release

COPY . .
# We have to touch a file so that the modification timestamp is different from
# our dummy program. If we don't, cargo doesn't rebuild the project and we end
# up with the output of the dummy program.
#
# https://github.com/rust-lang/cargo/issues/7982
RUN touch src/main.rs
RUN cargo build --locked --release


FROM debian:buster-slim

RUN apt-get update && apt-get install --no-install-recommends --yes \
    ca-certificates \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/* \
    && update-ca-certificates

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /zeroed-books

COPY --from=builder /usr/src/zeroed-books-api/target/release/zeroed-books-api ./
COPY templates ./templates

ENTRYPOINT [ "/zeroed-books/zeroed-books-api" ]
