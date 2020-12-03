import { Document, RenderedDocument } from './Document';
import React, { useState, useEffect, useCallback, useRef } from 'react';
import { Cluster, Cite, IncludeUncited } from '../../pkg';

import './bibliography.css';

const btnStyle = {
    display: "inline"
}

export const DocumentEditor = ({document, onChange, showBibliography}: { document: Document; onChange: (d: Document) => void, showBibliography: boolean }) => {
  let [csvIncl, setCsvIncl] = useState("");
    const insertCluster = (before: string) => {
        let neu = document.produce(draft => {
            let cl = draft.createCluster([{ id: 'citekey' }]);
            draft.insertCluster(cl, before);
        });
        onChange(neu);
    };
    let editors = document.clusters.map((cluster: Cluster) => {
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
          <div>
            Include uncited items?
              <input style={{width: 400}} type="text" value={csvIncl} onChange={(ev) => {
                let val = ev.target.value;
                let unc: IncludeUncited = "None";
                let joined = val;
                if (val == "All") {
                  unc = "All";
                } else if (val == "None") {
                  unc = "None";
                } else if (val == "") {
                  unc = "None";
                } else {
                  let arr = ev.target.value.split(/, */);
                  unc = { Specific: arr };
                  joined = arr.join(", ");
                }
                document.produce(d => d.setIncludeUncited(unc));
                setCsvIncl(joined);
              }} placeholder="None (default), All, or a comma-separated list of citekeys"/>
        </div>
        <DocumentViewer showBibliography={showBibliography} renderedDocument={document.rendered} />
    </>;
};

const DocumentViewer = React.memo(({renderedDocument, showBibliography}: {renderedDocument: RenderedDocument, showBibliography: boolean}) => {
    let clusters = renderedDocument.orderedClusterIds.map(c => {
        let html = renderedDocument.builtClusters[c.id];
        let touched = renderedDocument.updatedLastRevision[c.id];
        return <ClusterViewer key={c.id} note={c.note} html={html} touched={touched} />
    });
    let bib: JSX.Element | null = null;
    if (showBibliography) {
        let bibs = renderedDocument.bibliographyIds.map((key, x) => {
            let str = renderedDocument.bibliography[key];
            return <div key={x} className="footnote csl-entry" dangerouslySetInnerHTML={{__html: str}}></div>;
        });
        bib = <>
            <h2>Bibliography</h2>
            <div className="csl-bib-body">
              {bibs}
            </div>
        </>;
    }
    return <div>
        {clusters}
        {bib}
    </div>;
});

const ClusterViewer = React.memo(({note, html, touched}: { note: number | [number, number], html: string, touched: boolean }) => {
    let style = touched ? { backgroundColor: 'lightgoldenrodyellow' } : {};
    return <p className={"footnote"} style={style} dangerouslySetInnerHTML={{ __html: note + ". " + html }}></p>
});

const ClusterEditor = ({cluster, updateCluster, removeCluster}: {cluster: Cluster, removeCluster: () => void, updateCluster: (cluster: Cluster) => void}) => {
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

function useDidUpdateEffect(fn, inputs) {
  const didMountRef = useRef(false);

  useEffect(() => {
    if (didMountRef.current)
      fn();
    else
      didMountRef.current = true;
  }, inputs);
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

    useDidUpdateEffect(() => {
        update({ ...cite, id: key, locators: undefined, locator });
    }, [key, locator]);

    return <>
        <input value={key} onChange={upKey} />
        <input value={locator} onChange={upLocator} />
    </>;
}
