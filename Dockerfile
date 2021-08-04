FROM rust:1 as builder

WORKDIR /usr/src/zeroed-books-api

RUN rustup target add x86_64-unknown-linux-musl

# Create a dummy project and run a build with all our dependencies. As long as
# the dependencies don't change, any future builds will only have to recompile
# our code (and not the dependencies).
RUN cargo init
COPY Cargo.toml Cargo.lock ./
RUN cargo build --locked --release --target x86_64-unknown-linux-musl

COPY src src
# We have to touch a file so that the modification timestamp is different from
# our dummy program. If we don't, cargo doesn't rebuild the project and we end
# up with the output of the dummy program.
#
# https://github.com/rust-lang/cargo/issues/7982
RUN touch src/main.rs
RUN cargo build --locked --release --target x86_64-unknown-linux-musl


FROM scratch

COPY --from=builder /usr/src/zeroed-books-api/target/x86_64-unknown-linux-musl/release/zeroed-books-api /zeroed-books-api

ENTRYPOINT [ "/zeroed-books-api" ]
