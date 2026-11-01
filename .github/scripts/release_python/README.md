# Python Release Candidate Versions

The Python release workflow publishes release candidate tags to TestPyPI and
stable tags to PyPI.

Each source release keeps a stable Python version in
`bindings/python/Cargo.toml`. For a tag such as `v0.58.1-rc.5`, `prepare.py`
temporarily changes the PEP 621 metadata in `bindings/python/pyproject.toml`
from a dynamic Cargo version to the PEP 440 version `0.47.5rc5`. The workflow
runs this preparation independently before building the sdist and every wheel.
It does not commit the generated metadata or change the stable release version.

Stable tags skip this preparation, so maturin continues to read the version
from `bindings/python/Cargo.toml`.

Run the unit tests with:

```bash
python3 -m unittest discover -s .github/scripts/release_python -p "test_*.py"
```
