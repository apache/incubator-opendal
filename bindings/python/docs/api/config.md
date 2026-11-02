# Config

This page documents the typed service configurations in OpenDAL.

Each service exposes a `TypedDict` keyed on a `scheme` discriminant. Build one and
pass it to `Operator.from_config` / `AsyncOperator.from_config`:

```python
from opendal import Operator
from opendal.config import S3Config

op = Operator.from_config(S3Config(scheme="s3", bucket="my-bucket"))
```

::: opendal.config
    options:
      heading: "opendal.config"
      heading_level: 2
      show_source: false
