#!/bin/sh
# Container entrypoint: publish what is on the volume, then serve.
#
# Publishing a package is "drop the zip in and restart the unit";
# PACENOTES_GEN_ON_START=0 skips the regeneration.
set -eu

DATA_DIR="${PACENOTES_DATA_DIR:-/data}"

if [ "${PACENOTES_GEN_ON_START:-1}" = "1" ]; then
    if [ -d "$DATA_DIR" ]; then
        echo "entrypoint: regenerating $DATA_DIR/manifest.json"
        # A failed generation keeps the previous manifest, and the server starts anyway.
        pacenotes-server manifest gen --data "$DATA_DIR" \
            || echo "entrypoint: manifest gen FAILED — serving the previous manifest"
    else
        echo "entrypoint: $DATA_DIR does not exist — nothing to publish"
    fi
fi

exec pacenotes-server serve "$@"
