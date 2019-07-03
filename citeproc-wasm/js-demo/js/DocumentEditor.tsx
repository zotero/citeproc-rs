import { Document, stringifyInlines } from './Document';
import React, { useState } from 'react';
import { Cluster, Cite, Driver } from '../../pkg/citeproc_wasm';

export const DocumentEditor = ({clusters, driver}: {clusters: Cluster[], driver: Driver }) => {
    let [doc, setDocument] = useState(() => new Document(clusters, driver));
    let editors = clusters.map((cluster: Cluster) => {
        return <ClusterEditor key={cluster.id} cluster={cluster} updateCluster={(c) => {
            // TODO: setDocument with a new complete doc
        } } />
    });
    return <div>
        { editors }
        <DocumentViewer document={doc} />
    </div>;
};

const DocumentViewer = React.memo(({document}: {document: Document}) => {
    let clusters = document.clusters.map(c => {
        let html = document.builtClusters[c.id];
        return <ClusterViewer key={c.id} cluster={c} html={html} />
    });
    return <div>{clusters}</div>;
});

const ClusterViewer = React.memo(({cluster, html}: {cluster: Cluster, html: string }) => {
    return <p dangerouslySetInnerHTML={{ __html: cluster.noteNumber + ". " + html }}></p>
});

const ClusterEditor = ({cluster, updateCluster}: {cluster: Cluster, updateCluster: (cluster: Cluster) => void}) => {
    let [me, setMe] = useState(cluster);
    let editors = cluster.cites.map((cite: Cite) => {
        return <CiteEditor key={cite.citeId} cite={cite} update={(c) => {
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
    let [key, setKey] = useState(cite.id);
    const up = (s: string) => {
        setKey(s);
        let neu = { ...cite, id: s };
        update(neu);
    };
    return <div>
        <input value={key} onChange={(e) => up(e.target.value)} />
    </div>;
}