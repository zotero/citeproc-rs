import React, { Component, useState, useEffect, useRef } from 'react';
import { Result, Err, Ok, Option, Some } from 'safe-types';

import { Reference, Cluster, Driver, CslStyleError, StyleError, CiteprocRsError, CiteprocRsDriverError } from '../../pkg';
import { GraphViz } from './GraphViz';
import { DocumentEditor } from './DocumentEditor';
import { useDocument } from './useDocument';

import initialStyle from './style.csl';
import { initialReferences } from './initialReferences';
import { initialClusters } from './initialClusters';


const mono = {
    width: '100%',
    minHeight: '300px',
    fontFamily: 'monospace',
};

const StyleErrorViewer = ({style, message, error}: { style: string, message: string, error: StyleError }) => {
    switch (error.tag) {
        case "ParseError": 
            return <p>{ message }</p>
        case "DependentStyle":
            return <p>{ message }</p>
        case "Invalid":
            return <div>{ error.content.map(i => {
                let text = style.slice(i.range.start, i.range.end);
            return <div key={i.range.start * style.length + i.range.end}
            style={{backgroundColor: '#ff00002b', marginBottom: '5px'}}>
            <p>{ `${i.severity}: ${i.message}` }</p>
            <pre style={{marginLeft: "2em" }}>{ text }</pre>
            { i.hint && <p>{ i.hint }</p> }
            </div>;
        }) } </div>
        default: return null;
    }
}

const ErrorViewer = ({style, error}: { style: string, error: CiteprocRsError }) => {
    if (error instanceof CiteprocRsDriverError) {
        let info = error.data;
        switch (info.tag) {
            case "StyleError": return <StyleErrorViewer style={style} message={error.message} error={info.content} />;
            default: return <p>{ error.message }</p>;
        }
    } else {
        return <p>{ error.message }</p>;
    }
}

const Results = ({ driver, style }: { driver: Result<Driver, CiteprocRsError>, style: string }) => {
    return driver.match({
        Ok: d => <p>
            locales in use:
            <code>{JSON.stringify(d.toFetch().unwrap().sort())}</code>
            </p>,
        // Err: e => <p style={{backgroundColor: '#ff00002b', marginBottom: '5px'}}></p>,
        Err: e => <ErrorViewer style={style} error={e} />
    });
};

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

const EditorAndResults = () => {
    const {
        document,
        driver,
        style,
        setStyle,
        inFlight,
        setDocument,
        references,
        resetReferences,
        // updateReferences,
    } = useDocument(initialStyle, initialReferences, initialClusters);

    const docEditor = document.map(doc =>
        <DocumentEditor
        document={doc}
        onChange={newDoc => setDocument(Some(newDoc))} />
    ).unwrap_or(null);

    return <>
        <StyleEditor
            style={style}
            setStyle={setStyle}
            inFlight={inFlight}
            resetReferences={resetReferences} />
        <div style={{display: 'flex'}}>
            <section style={{flexGrow: 1}}>
                <Results style={style} driver={driver} />
                { docEditor }
            </section>
            <section style={{flexGrow: 1}}>
                <GraphViz references={references} driver={driver} />
            </section>
        </div>
        </>;
};

export default EditorAndResults;
