# Optional Blaze development

This entry point is an opt-in local experiment. It does not replace Cargo, CI, or release builds.
Install Blaze separately using its [official instructions](https://github.com/indexable-inc/blaze#install).
The script never installs tools or changes shell aliases. Missing Blaze returns exit status 127.

Run from the repository root:

```sh
./scripts/blaze-dev.sh build
./scripts/blaze-dev.sh test
./scripts/blaze-dev.sh test test_name

# Standard toolchain checks:
cargo build --locked -p mandoforge-api --bin mandoforge-api
cargo test --locked -p mandoforge-api --bin mandoforge-api
```

The script also works from another directory when invoked by its absolute path.
It selects only the named packages and debug/test targets. It rejects release and workspace flags.
It sets `BLAZE_TESTS=all` so repeated tests execute instead of reusing previous passing results.
Existing CI, release profiles, lockfiles, and toolchain pins remain unchanged.
Blaze uses its own Rust version; run the standard Cargo checks before committing.
Do not use Blaze results as release, performance, or production acceptance evidence.

## Measure before expanding

Use one checkout with no concurrent builds. Record the commit, Blaze version, and Cargo version.
Compare initial builds separately from warm builds after the same function-body edit.
Run the same test filter with each tool. Confirm matching results before comparing elapsed time and peak memory.
Restore the edited function after the comparison. Stop the daemon with `blaze stop` when finished.
On a 16 GiB Mac, start with one package and one active workspace.

No project-specific Blaze compatibility or speedup has been measured for this entry point.
See [Blaze compatibility](https://github.com/indexable-inc/blaze/blob/main/docs/cargo.md) for current restrictions.
