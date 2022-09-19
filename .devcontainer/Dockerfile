# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.177.0/containers/rust/.devcontainer/base.Dockerfile

FROM mcr.microsoft.com/vscode/devcontainers/rust:1

USER vscode
RUN rustup toolchain install stable --component clippy,rustfmt \
    && rustup default stable \
    && cargo install sqlx-cli --no-default-features --features native-tls,postgres
