- Proposal Name: `simulate_layer`
- Start Date: 2025-10-16
- RFC PR: [apache/opendal#6678](https://github.com/apache/opendal/pull/6678)
- Tracking Issue: [apache/opendal#6681](https://github.com/apache/opendal/issues/6681)

# Summary

Introduce a public `SimulateLayer` to replace the internal `CompleteLayer`, giving users explicit control over capability simulation while maintaining backward compatibility through a phased migration strategy.

# Motivation

Currently, OpenDAL automatically applies `CompleteLayer` internally to simulate missing capabilities for all backends. While this provides a seamless "batteries-included" experience, it has several limitations:

1. **Lack of user control**: Users cannot opt-out of simulations they don't need, even in performance-sensitive scenarios where native-only operations are preferred.

2. **Hidden complexity**: The `CompleteLayer` has grown to 385 lines handling multiple concerns (list recursive, stat dir, create_dir, reader/writer wrappers), making it difficult to maintain and understand.

3. **All-or-nothing approach**: Users either get all simulations or none. There's no way to selectively enable only the simulations they need.

4. **Binding limitations**: While Python/Java/Ruby bindings automatically benefit from simulations, they also cannot disable them, potentially causing unexpected behavior when users expect native backend semantics.

5. **Missing features**: Some useful simulations (like `start_after` for fs backend, reported in #6676) are not implemented because adding more logic to `CompleteLayer` would make it even more complex.

This proposal aims to:
- Make capability simulation **explicit and user-controllable**
- **Reduce complexity** by separating concerns
- Provide a **clear migration path** that maintains backward compatibility
- Enable **fine-grained control** over which simulations to apply

# Guide-level explanation

## For Rust users

After this change, capability simulation becomes explicit through `SimulateLayer`:

```rust
use opendal::layers::SimulateLayer;
use opendal::services::Fs;
use opendal::Operator;

// Default: all simulations enabled
let op = Operator::new(Fs::default())?
    .layer(SimulateLayer::default())
    .finish();

// Selective simulation with method chaining
let op = Operator::new(Fs::default())?
    .layer(
        SimulateLayer::default()
            .with_list_recursive(true)      // Enable recursive listing simulation
            .with_list_start_after(true)    // Enable start_after simulation
            .with_stat_dir(false)           // Disable stat dir simulation
            .with_create_dir(false)         // Disable create_dir simulation
    )
    .finish();

// Performance-critical: no simulation overhead
let op = Operator::new(S3::default())?
    // Don't add SimulateLayer - use native capabilities only
    .finish();
```

## For binding users (Python/Java/Ruby)

Binding users will see **no breaking changes** during the migration. Simulations will continue to work automatically:

```python
# Python - works the same way
operator = opendal.Operator("fs", root="/tmp")
files = list(operator.list("", start_after="file.txt"))  # Just works
```

## Migration path

### Phase 1: Next version (Soft transition - 0-3 months)
- `SimulateLayer` is introduced as a **public layer**
- `CompleteLayer` remains internal, controlled by `auto-simulate` feature (enabled by default)
- Documentation encourages explicit use of `SimulateLayer`
- No breaking changes

```toml
# Still works with auto-simulate enabled by default
[dependencies]
opendal = "0.x"
```

### Phase 2: Next 2-3 versions (Transition period - 3-6 months)
- Deprecation warnings added for implicit simulation
- Migration guide published
- Examples updated to use explicit `SimulateLayer`
- Users can test with `default-features = false` to disable auto-simulate

```toml
# Opt-out of auto-simulate to prepare for migration
[dependencies]
opendal = { version = "0.x", default-features = false }
```

### Phase 3: Next minor version after transition (Breaking change - 6 months)
- `auto-simulate` feature **disabled by default**
- Users must explicitly add `SimulateLayer` or opt-in with `features = ["auto-simulate"]`

```rust
// Required after this version
let op = Operator::new(Fs::default())?
    .layer(SimulateLayer::default())
    .finish();
```

### Phase 4: Next major version (Clean slate - 12 months)
- `CompleteLayer` and `auto-simulate` feature completely removed
- Bindings automatically apply `SimulateLayer` internally
- Core library requires explicit simulation

# Reference-level explanation

## Architecture changes

### Current architecture
```
Backend → CompleteLayer (auto-applied) → User
          ↓
          • list recursive simulation
          • stat dir simulation  
          • create_dir simulation
          • reader/writer wrappers
          • (all forced, no user control)
```

### New architecture
```
Backend → [SimulateLayer (user-controlled)] → User
          ↓
          • list recursive simulation (configurable)
          • list start_after simulation (configurable)
          • stat dir simulation (configurable)
          • create_dir simulation (configurable)
          • reader/writer wrappers (kept in CompleteLayer or removed)
```

## SimulateLayer implementation

```rust
// core/src/layers/simulate.rs

#[derive(Debug, Clone)]
pub struct SimulateLayer {
    list_recursive: bool,
    list_start_after: bool,
    list_limit: bool,        // Future
    stat_dir: bool,
    create_dir: bool,
}

impl Default for SimulateLayer {
    fn default() -> Self {
        Self {
            list_recursive: true,
            list_start_after: true,
            list_limit: false,
            stat_dir: true,
            create_dir: true,
        }
    }
}

impl SimulateLayer {
    /// Enable/disable recursive listing simulation (default: true)
    pub fn with_list_recursive(mut self, enabled: bool) -> Self {
        self.list_recursive = enabled;
        self
    }
    
    /// Enable/disable start_after simulation (default: true)
    pub fn with_list_start_after(mut self, enabled: bool) -> Self {
        self.list_start_after = enabled;
        self
    }
    
    /// Enable/disable stat directory simulation (default: true)
    pub fn with_stat_dir(mut self, enabled: bool) -> Self {
        self.stat_dir = enabled;
        self
    }
    
    /// Enable/disable create_dir simulation (default: true)
    pub fn with_create_dir(mut self, enabled: bool) -> Self {
        self.create_dir = enabled;
        self
    }
}
```

## Feature flag for migration

```rust
// core/src/types/operator/builder.rs

impl<A: Builder> Operator {
    pub fn new(accessor: A) -> OperatorBuilder<impl Access> {
        let builder = OperatorBuilder { accessor }
            .layer(ErrorContextLayer);
        
        #[cfg(feature = "auto-simulate")]
        let builder = builder.layer(CompleteLayer);
        
        builder.layer(CorrectnessCheckLayer)
    }
}

// Cargo.toml
[features]
default = ["auto-simulate"]  # Next version to Phase 3
# default = []               # Phase 3+ (breaking change)
auto-simulate = []
```

## Capability updates

Services that don't natively support certain features will have their `full_capability` updated when `SimulateLayer` is applied:

```rust
impl<A: Access> Layer<A> for SimulateLayer {
    fn layer(&self, inner: A) -> Self::LayeredAccess {
        let info = inner.info();
        
        info.update_full_capability(|mut cap| {
            if self.list_start_after && cap.list {
                cap.list_with_start_after = true;
            }
            if self.list_recursive && cap.list {
                cap.list_with_recursive = true;
            }
            if self.create_dir && cap.list && cap.write_can_empty {
                cap.create_dir = true;
            }
            cap
        });
        
        SimulateAccessor { 
            inner: Arc::new(inner), 
            info, 
            config: self.clone() 
        }
    }
}
```

## Bindings integration

Bindings will automatically apply `SimulateLayer` to maintain backward compatibility:

```rust
// bindings/python/src/operator.rs

impl Operator {
    pub fn new(scheme: &str, options: HashMap<String, String>) -> PyResult<Self> {
        let op = ocore::Operator::new(accessor)?
            .layer(ocore::layers::SimulateLayer::default())  // Auto-apply in bindings
            .finish();
        Ok(Self { core: op })
    }
}
```

## List start_after simulation

As a concrete example, here's how `start_after` will be simulated:

```rust
pub struct StartAfterLister<L> {
    inner: L,
    start_after: String,
    skipped: bool,
}

impl<L: oio::List> oio::List for StartAfterLister<L> {
    async fn next(&mut self) -> Result<Option<oio::Entry>> {
        loop {
            let Some(entry) = self.inner.next().await? else {
                return Ok(None);
            };
            
            // Skip entries until we find one > start_after
            if !self.skipped {
                if entry.path() <= self.start_after.as_str() {
                    continue;
                }
                self.skipped = true;
            }
            
            return Ok(Some(entry));
        }
    }
}
```

# Drawbacks

## Breaking change risk

Even with a phased migration, changing from implicit to explicit simulation is fundamentally a breaking change. Users who don't follow the migration guide will face compilation errors after Phase 3.

**Mitigation**: 
- 6-month transition period with clear deprecation warnings
- Comprehensive migration guide with automated tools
- `auto-simulate` feature for temporary backward compatibility

## Documentation burden

This change requires updating extensive documentation, examples, and tutorials across all bindings.

**Mitigation**:
- Automated documentation generation
- Version-specific migration guides
- Community support during transition

## Potential performance regression for some users

Users who upgrade without understanding the change might add `SimulateLayer::default()` everywhere, even for backends that don't need it, adding unnecessary overhead.

**Mitigation**:
- Clear documentation about which backends benefit from simulation
- Performance recommendations in the migration guide
- Compiler warnings for unnecessary simulations (future enhancement)

# Rationale and alternatives

## Why this design?

1. **Explicit over implicit**: Users should consciously decide what simulations to use, leading to better understanding and fewer surprises.

2. **Gradual migration**: The phased approach minimizes disruption while giving the community time to adapt.

3. **Binding compatibility**: Automatic application in bindings ensures that non-Rust users aren't affected.

4. **Extensibility**: The method chaining pattern makes it easy to add new simulations (like `list_with_limit`) without breaking changes.

## Alternative 1: Keep CompleteLayer, add disable flags

Instead of a new layer, add flags to disable specific simulations in `CompleteLayer`.

**Rejected because**:
- Doesn't address the "all-or-nothing" problem fundamentally
- Still forces all users to pay for simulation machinery even if disabled
- Doesn't improve code organization or maintainability

## Alternative 2: Service-specific default simulation

Each service declares which simulations it needs, applied automatically.

**Rejected because**:
- Less transparent - users don't know what's being simulated
- Service maintainers must decide defaults, which may not match user needs
- Doesn't allow per-application customization

## Alternative 3: Split into multiple layers

Create separate layers: `ListSimulateLayer`, `StatSimulateLayer`, `CreateDirSimulateLayer`.

**Rejected because**:
- Too granular for most users
- Verbose: `.layer(ListSimulateLayer).layer(StatSimulateLayer)...`
- Doesn't align with OpenDAL's "batteries included" philosophy
- `SimulateLayer` with method chaining provides the same granularity when needed

## Alternative 4: Builder pattern

Use a separate builder type like `SimulateLayer::builder().with_xxx().build()`.

**Rejected because**:
- More verbose than necessary for simple use cases
- Method chaining on the layer itself is more ergonomic
- OpenDAL already uses method chaining in many places (e.g., `OpList`)

## What if we don't do this?

1. `CompleteLayer` continues to grow in complexity
2. New simulations (like `start_after` for fs) remain unimplemented
3. Performance-sensitive users continue to be frustrated by forced overhead
4. OpenDAL becomes harder to maintain and understand

# Prior art

## Rust ecosystem

- **Tower** (HTTP middleware): Uses explicit middleware with builder pattern
- **Actix-web**: Middleware are opt-in with clear APIs
- **Reqwest**: Features are controlled via Cargo features, not runtime

## Other storage libraries

- **AWS SDK**: Capabilities are documented but not abstracted
- **GCS Client**: No simulation layer - users must handle missing features
- **MinIO SDK**: Provides compatibility helpers as separate modules

OpenDAL's approach is unique in providing transparent simulation, and this RFC aims to maintain that advantage while adding user control.

# Unresolved questions

## When to remove reader/writer wrappers?

`CompleteLayer` currently includes `CompleteReader` and `CompleteWriter` that validate read/write sizes. Should these:
1. Stay in a minimal `CompleteLayer` (still auto-applied)?
2. Move to `SimulateLayer`?
3. Move to a separate `ValidateLayer`?

**Proposed resolution**: Keep in minimal `CompleteLayer` during transition, revisit in next major version.

## Should we provide migration tooling?

Should we build `cargo-opendal migrate` or similar tools to automatically update code?

**Proposed resolution**: Defer to Phase 2, assess based on community feedback.

## Binding-specific configuration?

Should Python users be able to configure `SimulateLayer` through binding APIs?

```python
# Hypothetical API
operator = opendal.Operator(
    "fs", 
    root="/tmp",
    simulate={"list_start_after": True, "stat_dir": False}
)
```

**Proposed resolution**: Not in initial implementation. Can be added in future based on demand.

# Future possibilities

## Automatic simulation detection

Add compile-time or runtime warnings when `SimulateLayer` is used with backends that don't need it:

```rust
// Future enhancement
let op = Operator::new(S3::default())?
    .layer(SimulateLayer::default())  // Warning: S3 supports all features natively
    .finish();
```

## Conditional simulation based on operations

Allow enabling simulations only for specific operations:

```rust
// Future API
SimulateLayer::default()
    .simulate_only_for(&["list", "stat"])
```

## Simulation statistics

Expose metrics about which simulations are being used:

```rust
// Future API
let stats = op.simulation_stats();
println!("start_after simulated: {} times", stats.start_after_count);
```

## List limit simulation

Extend simulation to `list_with_limit` for backends that don't support it natively (requires client-side buffering).

## Versioning simulation

Extend simulation to `list_with_versions` for backends that don't support versioning natively (requires metadata storage).

## Plugin system for custom simulations

Allow users to provide custom simulation logic:

```rust
// Far future API
SimulateLayer::default()
    .with_custom_simulator(MyCustomSimulator::new())
```

## Zero-cost simulation layer

Use const generics or compile-time feature detection to make simulation truly zero-cost when all features are native:

```rust
// Speculative future optimization
let op = Operator::new(S3::default())?
    .layer(SimulateLayer::default())  // Optimized away at compile time for S3
    .finish();
```
