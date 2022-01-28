# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.177.0/containers/rust/.devcontainer/base.Dockerfile

FROM mcr.microsoft.com/vscode/devcontainers/rust:1

USER vscode
RUN rustup toolchain install stable --component clippy,rustfmt \
    && rustup default stable \
    && cargo install diesel_cli --no-default-features --features postgres \
    && echo 'eval "$(diesel completions bash)"' >> $HOME/.bashrc \
    && sudo mkdir -p /usr/share/zsh/vendor-functions \
    && diesel completions zsh | sudo tee /usr/share/zsh/vendor-functions/_diesel \
    # If we don't create this directory, the volume mount for storing GH
    # credentials creates it with root permissions, and VS code can't access it.
    && mkdir -p /home/vscode/.config
