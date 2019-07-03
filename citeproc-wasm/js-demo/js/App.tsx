import React, { Component, ChangeEvent, useEffect, useRef } from 'react';
import { asyncComponent } from 'react-async-component';
import { Driver as DriverT, Lifecycle, Reference, Cite, Cluster, Driver } from '../../pkg';
import { useState } from 'react';
import { DocumentEditor } from './DocumentEditor';
import { Result, Err, Ok, Option, Some, None } from 'safe-types';
import { useDocument } from './useDocument';

let initialStyle = `<style class="note">
  <features>
    <feature name="conditions" />
    <feature name="condition-date-parts" />
  </features>
  <citation et-al-min="3">
    <layout delimiter="; " suffix=".">
      <choose>
        <if position="ibid-with-locator">
          <group delimiter=", ">
            <text term="ibid" />
            <text variable="locator" />
          </group>
        </if>
        <else-if position="ibid">
          <text term="ibid" />
        </else-if>
        <else-if position="subsequent">
          <group delimiter=" ">
            <text variable="title" font-style="italic" />
            <text prefix="(n " variable="first-reference-note-number" suffix=")" />
          </group>
        </else-if>
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
            { citeId: 1, id: "citekey" }
        ],
        noteNumber: 1,
    },
    {
        id: 2,
        cites: [
            { citeId: 2, id: "citekey" }
        ],
        noteNumber: 2,
    },
    {
        id: 3,
        cites: [
            { citeId: 3, id: "citekey", locators: [["page", "56"]] }
        ],
        noteNumber: 3,
    },
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
        private cache: { [lang: string]: string } = {};
        async fetchLocale(lang: string) {
            if (typeof this.cache[lang] === 'string') {
                return this.cache[lang];
            }
            // this works
            // console.log(lang, "sleeping");
            // await sleep(400);
            // console.log(lang, "waking");
            let res = await fetch(`https://cdn.rawgit.com/citation-style-language/locales/master/locales-${lang}.xml`);
            if (res.ok) {
                let text = await res.text();
                this.cache[lang] = text;
                return text;
            }
        }
    }

    let fetcher = new Fetcher();

    function driverFactory(style: string): Result<DriverT, any> {
        try {
            let driver = Driver.new(style || initialStyle, fetcher);
            return Ok(driver);
        } catch (e) {
            console.log('caught error:', e)
            return Err(e);
        };
    };

    const StyleEditor = ({updateDriver, setReferences} : {
        inFlight: boolean,
        updateDriver: (s: Result<DriverT, any>) => void;
        setReferences: (rs: Reference[]) => void;
    }) => {
        const [text, setText] = useState(initialStyle);
        const [refsText, setRefsText] = useState(JSON.stringify(initialReferences, null, 2));

        const parse = () => {
            updateDriver(driverFactory(text));
        };

        const parseRefs = () => {
            try {
                let refs = JSON.parse(refsText);
                setReferences(refs);
            } catch (e) {
                console.error("could not parse references json", e);
            }
        };

        useEffect(parse, [ text ]);
        useEffect(parseRefs, [ refsText ]);

        const firstRun = useRef(true);
        if (firstRun.current) {
            firstRun.current = false;
            parse();
            parseRefs();
        }

        let column = { width: '50%' };
        return <div>
            <div style={{display: 'flex'}}>
                <div style={column}>
                    <h3>Style</h3>
                    <textarea value={text} onChange={(e) => setText(e.target.value)} style={mono} />
                </div>
                <div style={column}>
                    <h3>References</h3>
                    <textarea value={refsText} onChange={(e) => setRefsText(e.target.value)} style={mono} />
                </div>
            </div>
        </div>;
    }

    return StyleEditor;
}

const AsyncEditor = asyncComponent({
    resolve: loadEditor,
    LoadingComponent: () => <div><i>(Loading editor)</i></div>, // Optional
    ErrorComponent: ({ error }) => <pre>{JSON.stringify(error)}</pre> // Optional
});

const Results = ({ driver }: { driver: Result<Driver, any> }) => {
    return driver.match({
        Ok: d => <p>
            locales in use:
            <code>{JSON.stringify(d.toFetch())}</code>
        </p>,
        Err: e => <pre><code>{JSON.stringify(e, null, 2)}</code></pre>
    });
};

const App = () => {
    const {
        document,
        driver,
        updateDriver,
        inFlight,
        setDocument,
        resetReferences,
        updateReferences,
    } = useDocument(Err(undefined), initialReferences, initialClusters);

    const docEditor = document.map(doc =>
        <DocumentEditor
            document={doc}
            onChange={newDoc => setDocument(Some(newDoc))} />
    ).unwrap_or(null);

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
            <AsyncEditor updateDriver={updateDriver} inFlight={inFlight} setReferences={resetReferences} />
            <Results driver={driver} />
            { docEditor }
        </div>
    );
};


export default App;
