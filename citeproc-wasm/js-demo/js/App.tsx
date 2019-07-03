import React, { Component, ChangeEvent } from 'react';
import { asyncComponent } from 'react-async-component';
import { Driver as DriverT, Lifecycle, Reference, Cite, Cluster } from '../../pkg';
import { useState } from 'react';

type WasmPackage = typeof import ('../../pkg');

let initialStyle = `<style class="note">
  <features>
    <feature name="conditions" />
    <feature name="condition-date-parts" />
  </features>
  <citation et-al-min="3">
    <layout delimiter="; " suffix=".">
      <choose>
        <if position="ibid ibid-with-locator">
          <group delimiter=", ">
            <text term="ibid" />
            <text variable="locator" />
          </group>
        </if>
        <else>
          <group delimiter=", ">
            <text variable="title" font-style="italic" />
            <names variable="author" />
            <choose>
              <if>
                <conditions match="all">
                  <condition has-day="issued" />
                </conditions>
                <date variable="issued" form="numeric" />
              </if>
            </choose>
          </group>
        </else>
      </choose>
    </layout>
  </citation>
</style>`;

const initialReferences: Reference[] = [
    {
        id: 'citekey',
        type: 'book',
        author: [{ given: "Kurt", family: "Camembert" }],
        title: "Where The Vile Things Are",
        issued: { "raw": "1999-08-09" },
        language: 'fr-FR',
    },
    {
        id: 'foreign',
        type: 'book',
        title: "Some other title",
        language: 'fr-FR',
    }
];

const initialClusters: Cluster[] = [
    {
        id: 1,
        cites: [
            { citeId: 1, id: "foreign" },
            { citeId: 2, id: "citekey", locators: [["page", "56"]] }
        ],
        noteNumber: 1,
    }
];

const mono = {
    width: '100%',
    minHeight: '300px',
    fontFamily: 'monospace',
};

async function loadEditor() {
    const { Driver } = await import('../../pkg');

    function sleep(ms: number) {
        return new Promise(resolve => setTimeout(resolve, ms));
    }

    class Fetcher implements Lifecycle {
        async fetchLocale(lang: string) {
            // this works
            // console.log(lang, "sleeping");
            // await sleep(1000);
            // console.log(lang, "waking");
            let loc = '<?xml version="1.0" encoding="utf-8"?><locale xml:lang="' + lang + '"><terms><term name="edition">SUCCESS</term></terms></locale>';
            return loc;
        }
    }

    async function driverFactory(style: string): Promise<{driver: DriverT, error: any }> {
        try {
            let fetcher = new Fetcher();
            let driver = Driver.new(style || initialStyle, fetcher);
            driver.setReferences(initialReferences);
            driver.initClusters(initialClusters);
            await driver.fetchAll();
            return { driver, error: null };
        } catch (e) {
            console.log('caught error:', e)
            return {driver: null, error: e}
        };
    };

    const StyleEditor = ({updateDriver} : {updateDriver: (s: DriverState) => void}) => {
        const [text, setText] = useState(initialStyle);
        const [refsText, setRefsText] = useState(JSON.stringify(initialReferences, null, 2));
        const [oldDriver, setDriver] = useState(null as DriverT);
        const [inFlight, setInFlight] = useState(false);

        const parse = async () => {
            if (inFlight) { return };
            setInFlight(true);
            let { driver, error } = await driverFactory(text);
            try {
                const refs = JSON.parse(refsText);
                driver.setReferences(refs);
                await driver.fetchAll();
            } catch (e) {
                setInFlight(false);
                updateDriver({ driver: oldDriver, error: "could not set references" });
                return;
            }
            setInFlight(false);
            if (error) {
                updateDriver({ driver: null, error });
            }
            if (driver) {
                oldDriver && oldDriver.free();
                setDriver(driver);
                updateDriver({ driver, error: null })
            }
        };

        async function updateRefs(text: string) {
            setRefsText(text);
            if (oldDriver) {
                const refs = JSON.parse(text);
                oldDriver.setReferences(refs);
                updateDriver({ driver: oldDriver, error: null });
            }
        }

        if (!oldDriver) {
            parse();
        }

        return <div>
            <h3>Style</h3>
            <textarea value={text} onChange={(e) => setText(e.target.value)} style={mono} />
            <h3>References</h3>
            <textarea value={refsText} onChange={(e) => updateRefs(e.target.value)} style={mono} />
            <button disabled={inFlight} onClick={parse}>
                { !inFlight && "Parse" || "fetching locales" }
            </button>
        </div>;
    }

    return StyleEditor;

}

type DriverState = {
    driver: DriverT,
    error: any,
};

const Results = ({ driverState }: { driverState: DriverState }) => {
    const { driver, error } = driverState;
    return <div>
        {!error && driver && <p>languages in use: <code>{JSON.stringify(driver.toFetch())}</code></p>}
        {!error && driver && <p dangerouslySetInnerHTML={{__html: 
                stringifyInlines(driver.builtCluster(1)) || "render it here" 
            }}></p>}
        { error && <pre><code>{JSON.stringify(error, null, 2)}</code></pre> }
    </div>;
};

// Pandoc JSON won't be the output format forever -- when Salsa can do
// generics, then we will produce preformatted HTML strings.
interface Str { t: "Str", c: string };
interface Span { t: "Span", c: [any, Inline[]] };
interface Emph { t: "Emph", c: Inline[] };
interface Strikeout { t: "Strikeout", c: Inline[] };
interface Space { t: "Space" };
type Inline = Str | Space | Span | Emph | Strikeout;
function stringifyInlines(inlines: Inline[]): string {
    return inlines.map(inl => {
        switch (inl.t) {
            case "Str": return inl.c;
            case "Span": return "<span>" +stringifyInlines(inl.c) + '</span>';
            case "Emph": return "<i>" + stringifyInlines(inl.c) + "</i>";
            case "Space": return " ";
            default: return "\"" + inl.t + "\" AST node not supported"
        }
    }).join("");
}

const AsyncEditor = asyncComponent({
    resolve: loadEditor,
    LoadingComponent: () => <div><i>(Loading editor)</i></div>, // Optional
    ErrorComponent: ({ error }) => <pre>{JSON.stringify(error)}</pre> // Optional
});

const App = () => {
    const [driverState, setDriverState] = useState({ driver: null, error: null });
    return (
        <div className="App">
            <header className="App-header">
                <a
                    className="App-link"
                    href="https://github.com/cormacrelf/citeproc-rs"
                    target="_blank"
                    rel="noopener noreferrer"
                >
                    Test driver for <code>citeproc-wasm</code>
                </a>
            </header>
            <AsyncEditor updateDriver={setDriverState} />
            <Results driverState={driverState} />
        </div>
    );
};

export default App;
