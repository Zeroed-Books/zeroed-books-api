#!/usr/bin/env sh

# The devcontainer comes with a `vscode` user that we use to install software
# when building the container. At runtime, the ID of this user is adjusted to
# match the host user, but existing file permissions aren't changed. This causes
# permission errors when attempting to use `cargo`.

set -euf

# The `vscode` user always has an ID of 1000.
old_uid=1000
new_uid="$(id -un)"
new_gid="$(id -gn)"

fix_perms() {
    local target_dir="$1"

    echo "Changing ownership in '${target_dir}' from '${old_uid}' to '${new_uid}'..."
    sudo find "${target_dir}" -uid "${old_uid}" -exec chown "${new_uid}" {} +
    echo "Done!"
}

fix_perms "${CARGO_HOME}"
fix_perms "${RUSTUP_HOME}"

# When we mount the volume to store GitHub credentials, it gets created
# with root permissions. This means we can't authenticate with the `gh` CLI
# because it doesn't have permission to store the credentials file. Setting
# ownership fixes that.
sudo chown -R "${new_uid}:${new_gid}" /home/vscode/.config
