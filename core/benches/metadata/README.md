# Metadata benchmarks

This benchmark keeps only the final compact representation and replicas of the
field-per-option types that it replaced. The replicas preserve the old owned
field layout so the comparison remains available after the original types have
been removed.

Run the complete suite from `core/`:

```shell
cargo bench --bench metadata
```

Use a filter while iterating on one path:

```shell
cargo bench --bench metadata -- clone
```

The retained-value groups use Divan's allocation profiler. Construction groups
measure creation separately, while clone and lookup groups reuse an input value.
