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

echo "Changing ownership in '${CARGO_HOME}' from '${old_uid}' to '${new_uid}:${new_gid}'..."
sudo find "${CARGO_HOME}" -uid "${old_uid}" -exec chown "${new_uid}:${new_gid}" {} +
echo "Done!"
