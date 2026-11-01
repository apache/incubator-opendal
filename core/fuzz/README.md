# Fuzz Test for OpenDAL

fuzz test are used to test the robustness of the code. 

## Setup



To run the fuzz tests, please copy the `.env.example`, which is at project root, to `.env` and change the values on need.

Take `fs` for example, we need to change to enable fuzz test on `fs` on `/tmp`.

```dotenv
OPENDAL_TEST=fs
OPENDAL_FS_ROOT=/path/to/dir/
```

into

```dotenv
OPENDAL_TEST=fs
OPENDAL_FS_ROOT=/tmp/
```


## Run

List all fuzz targets.

```bash
cargo +nightly fuzz list
```

Build fuzz targets.

```bash
cargo +nightly fuzz build
```

Run a fuzz target(such as `reader`).

```bash
cargo +nightly fuzz run fuzz_reader
```

## Crash Reproduction

If you want to reproduce a crash, you first need to obtain the Base64 encoded code, which usually appears at the end of a crash report, and store it in a file.

Alternatively, if you already have the crash file, you can skip this step.

```bash
echo "Base64" > .crash
```

Print the `std::fmt::Debug` output for an input.

```bash
cargo +nightly fuzz fmt fuzz_target .crash
```

Rerun the fuzz test with the input.

```bash
cargo +nightly fuzz run fuzz_target .crash
```

For more details, please visit [cargo fuzz](https://rust-fuzz.github.io/book/cargo-fuzz/tutorial.html) or run the command cargo fuzz --help.
