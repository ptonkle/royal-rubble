# Vendored transport bundle

This directory holds one third-party file: a Trystero strategy bundle that introduces browsers over public Nostr relays. It is copied into the repository, pinned by hash, and never fetched at build time or at load.

| File | Version | License | sha256 |
| --- | --- | --- | --- |
| `trystero-nostr-0.25.4.js` | 0.25.4 | MIT | `6bfce15d72a64384cc66c2693917994e1b900f4f98c9b7a2d54e4e86f5202906` |

## Vendoring it

1. Download the self-contained ES module build of the strategy you want. Trystero publishes its strategies as npm packages, and a service such as esm.sh produces a single bundled module from one:

   ```bash
   curl -o trystero-nostr-0.25.4.js \
     https://esm.sh/@trystero-p2p/nostr@0.25.4/es2022/nostr.bundle.mjs
   ```

2. Open it and confirm it has no `import` of another URL and fetches nothing when it is imported. A bundle that imports from a CDN would make a request the CSP blocks.
3. Record its hash:

   ```bash
   shasum -a 256 trystero-nostr-0.25.4.js > SHA256SUMS
   ```

4. Add a row for it to the project's license list, and write the version and hash in the table above.
5. Add the file name to `NET_FILES` in `crates/core/tests/online_fence.rs`.

The same file with the same hash has been used by Floodline, Vagrancy, Mandible, Ribbit King and Level Twenty. Using that file again means one bundle to review.

## The build check

`scripts/build.sh` refuses to build when the bundle differs from its recorded hash. Add these lines before the WebAssembly build:

```bash
# The vendored transport bundle must be the one pinned (web/net/vendor/README.md).
(cd web/net/vendor && shasum -a 256 -c SHA256SUMS >/dev/null) || {
  echo "web/net/vendor does not match SHA256SUMS" >&2
  exit 1
}
```

On Linux without `shasum`, use `sha256sum -c SHA256SUMS`. The template's `cp web/* dist/` copies only the top level of `web/`; change it to `cp -R web/* dist/` so `dist/net/` is built too.

## Upgrading

An upgrade is a decision, recorded in `SECOND-ORDER.md`: download the new version, read it, replace the file and its hash, update `NET_FILES` and `bundle` in `config.js`, and run `tests/browser/online.py` and the room-code check by hand. A new version may change its default relays. The kit passes its own list from `config.js`, so the bundle's defaults are never used.
