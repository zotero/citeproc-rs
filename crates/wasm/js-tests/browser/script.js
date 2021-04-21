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
    await wasm_bindgen('./pkg-nomod/_no_modules/citeproc_rs_wasm_bg.wasm');

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
}

run()
    .then(() => document.write('<p id="success">success</p>'))
    .catch((e) => document.write('<p id="failure">' + e.message + '</p>'));
