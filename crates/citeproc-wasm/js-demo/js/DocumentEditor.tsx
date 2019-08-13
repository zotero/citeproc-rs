import { Document, RenderedDocument } from './Document';
import React, { useState, useCallback } from 'react';
import { Cluster, Cite } from '../../pkg';

export const DocumentEditor = ({document, onChange}: { document: Document; onChange: (d: Document) => void }) => {
    const insertCluster = (before: number) => {
        let neu = document.produce(draft => {
            let cl = draft.createCluster([{ id: 'citekey' }]);
            draft.insertCluster(cl, before);
        });
        onChange(neu);
    };
    let editors = document.clusters.map((cluster: Cluster) => {
        return <div key={cluster.id}>
            <button onClick={() => insertCluster(cluster.id)} >+cluster</button>
            <ClusterEditor
                key={cluster.id}
                cluster={cluster}
                updateCluster={ newCluster => onChange(document.produce(d => d.replaceCluster(newCluster))) } />
        </div>;
    });
    return <>
        { editors }
        <button onClick={() => insertCluster(null)} >+cluster</button>
        <DocumentViewer renderedDocument={document.rendered} />
    </>;
};

const DocumentViewer = React.memo(({renderedDocument}: {renderedDocument: RenderedDocument}) => {
    let clusters = renderedDocument.orderedClusterIds.map(c => {
        let html = renderedDocument.builtClusters[c.id];
        let touched = renderedDocument.updatedLastRevision[c.id];
        return <ClusterViewer key={c.id} noteNumber={c.noteNumber} html={html} touched={touched} />
    });
    return <div>{clusters}</div>;
});

const ClusterViewer = React.memo(({noteNumber, html, touched}: { noteNumber: number, html: string, touched: boolean }) => {
    let style = touched ? { backgroundColor: 'lightgoldenrodyellow' } : {};
    return <p style={style} dangerouslySetInnerHTML={{ __html: noteNumber + ". " + html }}></p>
});

const ClusterEditor = ({cluster, updateCluster}: {cluster: Cluster, updateCluster: (cluster: Cluster) => void}) => {
    let [me, setMe] = useState(cluster);
    let editors = cluster.cites.map((cite: Cite) => {
        return <CiteEditor key={cite.citeId} cite={cite} update={ c => {
            let cites = me.cites.slice(0) as Cite[];
            cites[cites.findIndex(x => x.citeId === c.citeId)] = c;
            let _me = { ...me, cites };
            setMe(_me);
            updateCluster(_me);
        } } />
    });
    return <div>
        { editors }
        {/* <textarea value={text} onChange={(e) => setText(e.target.value)} style={mono} /> */}
    </div>;
}

const CiteEditor = ({cite, update}: {cite: Cite, update: (cite: Cite) => void}) => {
    let initT: string, initL: string;
    if (cite.locators && cite.locators.length > 0) {
        [initT, initL] = cite.locators[0];
    }
    let [key, setKey] = useState(cite.id);
    let [locType, setLocType] = useState(initT);
    let [locator, setLocator] = useState(initL);
    const up = (k?: string) => {
        if (k) setKey(k);
        update({ ...cite, id: k });
    };
    const upLocator = useCallback((l: string, ty = "page") => {
        setLocator(l);
        setLocType(ty);
        let neu: [string, string][] = l ? [[ty, l]] : undefined;
        update({ ...cite, locators: neu });
    }, []);
    return <div>
        <input value={key} onChange={(e) => up(e.target.value)} />
        <input value={locator} onChange={(e) => upLocator(e.target.value)} />
    </div>;
}
