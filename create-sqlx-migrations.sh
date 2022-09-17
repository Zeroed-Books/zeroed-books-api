#!/usr/bin/env bash

set -euf
set -o pipefail

src_migrations=./migrations
dest_migrations=./migrations-sqlx

rm -rf "${dest_migrations}"
mkdir "${dest_migrations}"

while IFS= read -r d; do
    echo $d

    cleaned_name="${d//-/}"
    cp "${src_migrations}/${d}/down.sql" "${dest_migrations}/${cleaned_name}.down.sql"
    cp "${src_migrations}/${d}/up.sql" "${dest_migrations}/${cleaned_name}.up.sql"
done < <(cd "${src_migrations}" && find -path './*' -prune -type d)
