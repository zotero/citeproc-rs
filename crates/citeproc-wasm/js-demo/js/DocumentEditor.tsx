import { Document, RenderedDocument } from './Document';
import React, { useState, useEffect, useCallback } from 'react';
import { NoteCluster, Cite } from '../../pkg';

const btnStyle = {
    display: "inline"
}

export const DocumentEditor = ({document, onChange}: { document: Document; onChange: (d: Document) => void }) => {
    const insertCluster = (before: number) => {
        let neu = document.produce(draft => {
            let cl = draft.createCluster([{ id: 'citekey' }]);
            draft.insertCluster(cl, before);
        });
        onChange(neu);
    };
    let editors = document.clusters.map((cluster: NoteCluster) => {
        return <div key={cluster.id}>
            <button style={btnStyle} onClick={() => insertCluster(cluster.id)} >+cluster</button>
            <ClusterEditor
                key={cluster.id}
                cluster={cluster}
                removeCluster={ () => onChange(document.produce(d => d.removeCluster(cluster.id))) }
                updateCluster={ newCluster => onChange(document.produce(d => d.replaceCluster(newCluster))) } />
        </div>;
    });
    return <>
        { editors }
        <button style={btnStyle} onClick={() => insertCluster(null)} >+cluster</button>
        <DocumentViewer renderedDocument={document.rendered} />
    </>;
};

const DocumentViewer = React.memo(({renderedDocument}: {renderedDocument: RenderedDocument}) => {
    let clusters = renderedDocument.orderedClusterIds.map(c => {
        let html = renderedDocument.builtClusters[c.id];
        let touched = renderedDocument.updatedLastRevision[c.id];
        return <ClusterViewer key={c.id} note={c.note} html={html} touched={touched} />
    });
    return <div>{clusters}</div>;
});

const ClusterViewer = React.memo(({note, html, touched}: { note: number | [number, number], html: string, touched: boolean }) => {
    let style = touched ? { backgroundColor: 'lightgoldenrodyellow' } : {};
    return <p className={"footnote"} style={style} dangerouslySetInnerHTML={{ __html: note + ". " + html }}></p>
});

const ClusterEditor = ({cluster, updateCluster, removeCluster}: {cluster: NoteCluster, removeCluster: () => void, updateCluster: (cluster: NoteCluster) => void}) => {
    let [me, setMe] = useState(cluster);
    let editors = cluster.cites.map((cite: Cite, i) => {
        return <CiteEditor key={i} cite={cite} update={ c => {
            let cites = me.cites.slice(0) as Cite[];
            cites[i] = c;
            let _me = { ...me, cites };
            setMe(_me);
            updateCluster(_me);
        } } />
    });
    editors.push(
        <button style={btnStyle} key="removeCluster" onClick={removeCluster} >-cluster</button>
    );
    return <>
        { editors }
        {/* <textarea value={text} onChange={(e) => setText(e.target.value)} style={mono} /> */}
    </>;
}

const CiteEditor = ({cite, update}: {cite: Cite, update: (cite: Cite) => void}) => {
    let [key, setKey] = useState(cite.id);
    let [locator, setLocator] = useState(cite.locator);

    const upKey = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
        let k = e.target.value;
        setKey(k);
    }, []);
    const upLocator = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
        let l = e.target.value;
        if (l == "") {
            l = undefined;
        }
        setLocator(l);
    }, []);

    useEffect(() => {
        update({ ...cite, id: key, locators: undefined, locator });
    }, [key, locator]);

    return <>
        <input value={key} onChange={upKey} />
        <input value={locator} onChange={upLocator} />
    </>;
}
