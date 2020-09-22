import React, { Component, ChangeEvent, useEffect, useRef } from 'react';
import { asyncComponent } from 'react-async-component';
import { Reference, Cluster, Driver, StyleError, ParseError, Invalid } from '../../pkg';
import { useState } from 'react';
import { DocumentEditor } from './DocumentEditor';
import { GraphViz } from './GraphViz';
import { Result, Err, Ok, Option, Some, None } from 'safe-types';
import { useDocument } from './useDocument';

import initialStyle from './style.csl';
import { initialReferences } from './initialReferences.ts';

const initialClusters: Cluster[] = [
    {
        id: 1,
        cites: [
            { id: "citekey" }
        ],
    },
    {
        id: 2,
        cites: [
            { id: "citekey2" }
        ],
    },
    {
        id: 3,
        cites: [
            { id: "citekey", locator: "56" }
        ],
    },
    {
        id: 4,
        cites: [
            { id: "r1" }
        ]
    },
    {
        id: 5,
        cites: [
            { id: "ysuf1" }
        ]
    },
    {
        id: 6,
        cites: [
            { id: "ysuf2" }
        ]
    },
    {
        id: 7,
        cites: [
            { id: "ysuf1" }
        ]
    },
    {
        id: 8,
        cites: [
            { id: "r7" }
        ]
    },
];

const mono = {
    width: '100%',
    minHeight: '300px',
    fontFamily: 'monospace',
};

async function loadEditor() {
    // Load wasm before making it interactive.
    // Removes failed expectation of immediate response compared to lazily loading it.
    await import('../../pkg');

    const StyleEditor = ({style, setStyle, resetReferences} : {
        inFlight: boolean,
        style: string,
        setStyle: React.Dispatch<string>,
        resetReferences: (rs: Reference[]) => void;
    }) => {
        const [refsText, setRefsText] = useState(JSON.stringify(initialReferences, null, 2));

        const parseRefs = () => {
            try {
                let refs = JSON.parse(refsText);
                resetReferences(refs);
            } catch (e) {
                console.error("could not parse references json", e);
            }
        };

        useEffect(parseRefs, [ refsText ]);

        const firstRun = useRef(true);
        if (firstRun.current) {
            firstRun.current = false;
            parseRefs();
        }

        let column = { width: '50%' };
        return <div>
            <div style={{display: 'flex'}}>
                <div style={column}>
                    <h3>Style</h3>
                    <textarea value={style} onChange={e => setStyle(e.target.value)} style={mono} />
                </div>
                <div style={column}>
                    <h3>References</h3>
                    <textarea value={refsText} onChange={e => setRefsText(e.target.value)} style={mono} />
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

const Results = ({ driver, style }: { driver: Result<Driver, any>, style: string }) => {
    return driver.match({
        Ok: d => <p>
            locales in use:
            <code>{JSON.stringify(d.toFetch().sort())}</code>
        </p>,
        Err: e => <p style={{backgroundColor: '#ff00002b', marginBottom: '5px'}}>{e}</p>,
    });
};



const ErrorViewer = ({style, error}: { style: string, error: StyleError }) => {
    if (error.ParseError) {
        let e = error as ParseError;
        return <p>{ e.ParseError }</p>
    } else if (error.Invalid) {
        let e = error as Invalid;
        return <div>{ e.Invalid.map(i => {
            let text = style.slice(i.range.start, i.range.end);
            return <div key={i.range.start * style.length + i.range.end}
                        style={{backgroundColor: '#ff00002b', marginBottom: '5px'}}>
                <p>{ `${i.severity}: ${i.message}` }</p>
                <pre style={{marginLeft: "2em" }}>{ text }</pre>
                { i.hint && <p>{ i.hint } </p>}
            </div>
        }) } </div>
    } else {
        return null;
    }
}

const App = () => {
    const {
        document,
        driver,
        style,
        setStyle,
        inFlight,
        setDocument,
        references,
        resetReferences,
        updateReferences,
    } = useDocument(initialStyle, initialReferences, initialClusters);

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
            <AsyncEditor style={style} setStyle={setStyle} inFlight={inFlight} resetReferences={resetReferences} />
            <div  style={{display: 'flex'}}>
                <section style={{flexGrow: 1}}>
                    <Results style={style} driver={driver} />
                    { docEditor }
                </section>
                <section style={{flexGrow: 1}}>
                    <GraphViz references={references} driver={driver} />
                </section>
            </div>
        </div>
    );
};


export default App;
