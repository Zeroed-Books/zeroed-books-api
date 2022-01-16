# See here for image contents: https://github.com/microsoft/vscode-dev-containers/tree/v0.177.0/containers/rust/.devcontainer/base.Dockerfile

FROM mcr.microsoft.com/vscode/devcontainers/rust:1

RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends vim

USER vscode
RUN cargo install diesel_cli --no-default-features --features postgres \
    && echo 'eval "$(diesel completions bash)"' >> $HOME/.bashrc \
    && sudo mkdir -p /usr/share/zsh/vendor-functions \
    && diesel completions zsh | sudo tee /usr/share/zsh/vendor-functions/_diesel
