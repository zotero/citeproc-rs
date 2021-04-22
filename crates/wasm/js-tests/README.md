## Running tests

Requires bash. Sorry Windows.

```sh
yarn
yarn build
./install_firefox.sh
yarn test
```

Running `yarn`/`yarn install` will delete the `node_modules/@citeproc-rs/wasm`
folder, so run `yarn build` again.

## Disabling the Firefox ESR integration test

```sh
yarn
yarn build
yarn test-node-only # this force-disables.
yarn jest # runs it if FIREFOX_BINARY_PATH is set to a firefox binary
```

