
// Use ES module import syntax to import functionality from the module
// that we have compiled.
//
// Note that the `default` import is an initialization function which
// will "boot" the module and make it ready to use. Currently browsers
// don't support natively imported WebAssembly as an ES module, but
// eventually the manual initialization won't be required!
//
// Specifically for citeproc-rs, note that we load the JS file from the
// _web directory. The other directories are for different environments
// like Node and WebPack. You could also load from cdnjs or similar with a
// version number and the /_web/citeproc_rs_wasm.js file path.
console.log("first");
const { Driver } = wasm_bindgen;

const mkStyle = (inner, bibliography) => {
    return `
    <style class="note">
      <info>
        <id>https://github.com/cormacrelf/citeproc-rs/test-style</id>
        <title>test-style</title>
        <updated>2000-01-01T00:00:00Z</updated>
      </info>
      <citation>
        <layout>
          ${inner}
        </layout>
      </citation>
      ${ bibliography != null ? bibliography : "" }
    </style>
    `;
}

let style = mkStyle();


class Fetcher {
    async fetchLocale(lang) {
        // We're going to return a sentinel value so we know the french locale is getting loaded
        let loc = '<?xml version="1.0" encoding="utf-8"?><locale xml:lang="' + lang + '"><terms><term name="edition">édition (fr)</term></terms></locale>';
        return loc;
    }
}

async function run() {
    // First up we need to actually load the wasm file, so we use the
    // default export to inform it where the wasm file is located on the
    // server, and then we wait on the returned promise to wait for the
    // wasm to be loaded.
    //
    // It may look like this: `await init('./pkg/without_a_bundler_bg.wasm');`,
    // but there is also a handy default inside `init` function, which uses
    // `import.meta` to locate the wasm file relatively to js file.
    //
    // Note that instead of a string you can also pass in any of the
    // following things:
    //
    // * `WebAssembly.Module`
    //
    // * `ArrayBuffer`
    //
    // * `Response`
    //
    // * `Promise` which returns any of the above, e.g. `fetch("./path/to/wasm")`
    //
    // This gives you complete control over how the module is loaded
    // and compiled.
    //
    // Also note that the promise, when resolved, yields the wasm module's
    // exports which is the same as importing the `*_bg` module in other
    // modes
    await wasm_bindgen('../../pkg-nomod/_no_modules/citeproc_rs_wasm_bg.wasm');

    // And afterwards we can use all the functionality defined in wasm.
    // const result = Driver.new("<>noparse", {}, 2);
    const fetcher = new Fetcher();
    const driver = Driver.new({
        style,
        fetcher,
        format: "html"
    }).unwrap();

    console.log("--- Successfully loaded wasm driver. You can now use it. ---")
    driver.insertReference({id: "citekey", title: "Hello", language: 'fr-FR'}).unwrap();
    driver.initClusters([{id: "one", cites: [{id: "citekey"}]}]).unwrap();
    driver.setClusterOrder([ {id: "one"} ]).unwrap();
    await driver.fetchLocales();
    let result = driver.builtCluster("one").unwrap();
    console.log("Built a cite cluster:", result);
    return;
}

run()
    .then(() => document.write('<p id="success">success</p>'))
    .catch((e) => document.write('<p id="failure">' + e.message + '</p>'));
