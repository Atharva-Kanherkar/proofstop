# Archived repository controls

`h11-readers-fixed.py` is the exact `h11/_readers.py` from python-hyper/h11 commit
`1c5b07581f058886c8bdd87adababd7d959dc7ca` (v0.16.0). It is an upstream MIT-licensed
control, used only for verification. The license is preserved at
`tasks/repository/h11-chunk-footer/repo/LICENSE.txt`.

This file is never supplied to model generation. Replacing the target with this
file while keeping the v0.14.0 runtime unchanged tests the narrow parser fix.
