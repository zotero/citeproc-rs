// https://github.com/GoogleChromeLabs/wasm-feature-detect
import * as features from "https://unpkg.com/wasm-feature-detect@1.2.10?module";
const { Driver } = wasm_bindgen;

const mkNoteStyle = (inner, bibliography) => {
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
      ${bibliography != null ? bibliography : ""}
    </style>
    `;
}

let style = mkNoteStyle();


class Fetcher {
    async fetchLocale(lang) {
        // We're going to return a sentinel value so we know the french locale is getting loaded
        let loc = '<?xml version="1.0" encoding="utf-8"?><locale xml:lang="' + lang + '"><terms><term name="edition">édition (fr)</term></terms></locale>';
        return loc;
    }
}

async function test_citeproc_rs() {
    await wasm_bindgen('./pkg-nomod/_no_modules/citeproc_rs_wasm_bg.wasm');

    const fetcher = new Fetcher();
    const driver = new Driver({
        style,
        fetcher,
        format: "html"
    });

    console.log("--- Successfully loaded wasm driver. You can now use it. ---")
    driver.insertReference({ id: "citekey", title: "Hello", language: 'fr-FR' });
    driver.initClusters([{ id: "one", cites: [{ id: "citekey" }] }]);
    driver.setClusterOrder([{ id: "one" }]);
    await driver.fetchLocales();
    let result = driver.builtCluster("one");
    console.log("Built a cite cluster:", result);
}

// this tests the current Firefox ESR's wasm support
async function test_wasm_support() {
    let allChecks = await Promise.all(
        Object.keys(features).map(async name => {
            let feat = features[name];
            let supported = await feat();
            return { name, supported }
        })
    );

    let resolved = {};
    allChecks.forEach((check) => {
        let { name, supported } = check;
        resolved[name] = supported;
    });

    // ESR 60.9 supports none of the newer features, but
    // you would expect to have to change a few of these
    // when newer ESRs are used in Zotero.
    let expected = {
        mutableGlobals: false,
        bigInt: false,
        bulkMemory: false,
        exceptions: false,
        memory64: false,
        multiValue: false,
        referenceTypes: false,
        saturatedFloatToInt: false,
        signExtensions: false,
        simd: false,
        tailCall: false,
        threads: false,
    };

    let results = Object.keys(resolved).map(key => {
        return {
            key,
            expected: expected[key],
            resolved: resolved[key],
        };
    });

    let failed = results.filter(x => x.expected !== x.resolved);
    if (failed.length > 0) {
        throw new Error("wasm support mismatch: " + JSON.stringify(failed))
    }

}

async function suite() {
    await test_citeproc_rs();
    await test_wasm_support();
}

suite()
    .then(() => document.write('<p id="success">success</p>'))
    .catch((e) => document.write('<p id="failure">' + e.message + '</p>'));
