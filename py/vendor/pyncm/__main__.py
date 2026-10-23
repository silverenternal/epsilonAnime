"""Vendored pyncm CLI is intentionally not included.

epsilonAnime imports the ``pyncm`` library APIs directly. The upstream
standalone downloader lived in a 7k-line generated module; keeping it would
violate this repository's 600-line source-file limit.
"""

raise SystemExit("pyncm CLI is not vendored; use the epsilonAnime backend")
