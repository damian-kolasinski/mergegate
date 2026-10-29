"""Kept for the wakes Codex agents left before `mg wakes codex`, which run
this file by its path: it hands over to `mg wakes codex`. The hub puts its
own `mg` first on a wake's PATH."""
import os
import sys

os.execvp("mg", ["mg", "wakes", "codex", *sys.argv[1:]])
