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
        # The server starts either way; exit 2 means written with packages withheld.
        status=0
        pacenotes-server manifest gen --data "$DATA_DIR" || status=$?
        if [ "$status" -ne 0 ]; then
            echo "=================================================================="
            if [ "$status" -eq 2 ]; then
                echo "entrypoint: manifest gen PUBLISHED WITH ERRORS (exit 2)."
                echo "  Some packages were withheld — read the ERROR lines above."
            else
                echo "entrypoint: manifest gen FAILED (exit $status)."
                echo "  Nothing was written; the PREVIOUS manifest is being served."
            fi
            echo "=================================================================="
        fi
    else
        echo "entrypoint: $DATA_DIR does not exist — nothing to publish"
    fi
fi

exec pacenotes-server serve "$@"
