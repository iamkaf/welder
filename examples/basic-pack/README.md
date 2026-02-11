# Welder example: basic-pack

This is a tiny example asset pack project you can use to sanity-check Welder.

## Run

From the repo root:

```bash
# build the tool
cargo build

# run the pipeline in the example project
cd examples/basic-pack
../../target/debug/welder doctor
../../target/debug/welder build
../../target/debug/welder preview
../../target/debug/welder package
../../target/debug/welder publish --dry-run
```

Outputs are written under `examples/basic-pack/dist/`.
