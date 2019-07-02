# 🦀 Demo app based on [`rust-webpack-template`][tmpl]

[tmpl]: (https://github.com/rustwasm/rust-webpack-template)

This has:

* TypeScript, with the exported definitions from `citeproc-wasm`
* React
* Webpack

## 🔋 Batteries Included

This template comes pre-configured with all the boilerplate for recompiling 
`citeproc-wasm` automatically and hooking it into a Webpack build pipeline. If 
you are just using `citeproc-wasm`, your needs will be different and you'll 
likely just want to import an npm package instead.

Note that at the moment, importing Rust WebAssembly packages means using async 
imports, as below. This demo is an example.

```typescript
import('citeproc-wasm').then(({ Driver }) => {
    const driver = new Driver(...);
})
```

## How to try it

* `yarn` -- Install packages.

* `yarn start` -- Serve the project locally for development at
  `http://localhost:8080`.

* `yarn build` -- Bundle the project (in production mode) into `dist/`.

