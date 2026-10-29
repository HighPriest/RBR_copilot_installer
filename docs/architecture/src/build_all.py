"""Regenerate every architecture diagram in this folder.

    python docs/architecture/src/build_all.py

Needs Python 3 and Pillow. No graphviz, no mermaid, no browser: the layout is
explicit coordinates in the modules below, which is why the pictures are
reproducible anywhere.
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import activity_installer  # noqa: E402
import er  # noqa: E402
import server_activity  # noqa: E402
import sysml  # noqa: E402

OUT = os.path.dirname(HERE)


def main():
    written = [
        er.server(OUT),
        er.installer(OUT),
        activity_installer.installer_run(OUT),
        activity_installer.engine_run(OUT),
        server_activity.server_request(OUT),
        server_activity.manifest_gen(OUT),
        sysml.bdd(OUT),
        sysml.ibd(OUT),
        sysml.requirements(OUT),
    ]
    for path in written:
        print(path)


if __name__ == "__main__":
    main()
