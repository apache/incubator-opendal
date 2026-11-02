## Capabilities

This service can be used to:

- [ ] create_dir
- [x] stat
- [x] read
- [x] write
- [x] delete
- [x] list
- [ ] copy
- [ ] rename
- [ ] presign

## Example

### Via Builder


```rust,no_run
use std::sync::Arc;

use anyhow::Result;
use opendal_core::services::Memory;
use opendal_core::Operator;

#[tokio::main]
async fn main() -> Result<()> {
    let mut builder = Memory::default().root("/tmp");

    let op: Operator = Operator::new(builder)?;

    Ok(())
}
```
