import React, { useState, useEffect, useRef } from 'react';
import { Result, Some } from 'safe-types';
import ReactJson from 'react-json-view';

import { Reference, Driver, StyleError } from '../../pkg';
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

const errorStyle = {backgroundColor: '#ff00002b', marginBottom: '5px'};

const StyleErrorViewer = ({style, message, error}: { style: string, message: string, error: StyleError }) => {
    switch (error.tag) {
        case "Invalid":
            return <div>{
                error.content.map(i => {
                    let text = style.slice(i.range.start, i.range.end);
                    return <div key={i.range.start * style.length + i.range.end}
                        style={errorStyle}>
                        <p>{ `${i.severity}: ${i.message}` }</p>
                        <pre style={{marginLeft: "2em" }}>{ text }</pre>
                        { i.hint && <p>{ i.hint }</p> }
                    </div>;
                })
            } </div>;
        default:
            return <p style={errorStyle}>{ message }</p>
    }
}

const ErrorViewer = ({style, error}: { style: string, error: CiteprocRsError }) => {
    if (error instanceof CslStyleError) {
        let info = error.data;
        return <StyleErrorViewer
                    style={style}
                    message={error.message}
                    error={info} />;
    } else {
        return <p style={errorStyle}>{ error.message }</p>;
    }
}

const Results = ({ driver, style }: { driver: Result<Driver, CiteprocRsError>, style: string }) => {
    return driver.match({
        Ok: d => <p>
            locales in use:
            <code>{JSON.stringify(d.toFetch().sort())}</code>
            </p>,
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
                <textarea defaultValue={style} onBlur={e => setStyle(e.target.value)} style={mono} />
            </div>
            <div style={column}>
                <h3>References</h3>
                <textarea defaultValue={refsText} onBlur={e => setRefsText(e.target.value)} style={mono} />
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
        metadata,
        // updateReferences,
    } = useDocument(initialStyle, initialReferences, initialClusters);

    let [hasBibliography, setHasBibliography] = useState(false);
    useEffect(() => {
        metadata.map(meta => {
            if (meta.independentMeta) {
                setHasBibliography(meta.independentMeta.hasBibliography);
            }
        })
    }, [metadata]);

    const docEditor = document.map(doc =>
        <DocumentEditor
        showBibliography={hasBibliography}
        document={doc}
        onChange={newDoc => setDocument(Some(newDoc))} />
    ).unwrap_or(null);

    return <>
        <StyleEditor
            style={style}
            setStyle={setStyle}
            inFlight={inFlight}
            resetReferences={resetReferences} />
        <section>
          { metadata.map_or(null, meta => <details>
              <summary>Style Metadata</summary>
              <ReactJson name={false} src={meta}
                enableClipboard={false} displayObjectSize={false} displayDataTypes={false} />
            </details>) }
        </section>
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
