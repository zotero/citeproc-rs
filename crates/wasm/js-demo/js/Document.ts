import { Reference, Cite, Cluster, ClusterPosition, Driver, UpdateSummary, Lifecycle } from '../../pkg';
import { produce, immerable, Draft, IProduce } from 'immer';

export type ClusterId = number;
export type CiteId = number;

export class RenderedDocument {

    /** Caches HTML for a ClusterId, that is pulled from the driver */
    public builtClusters: { [id: number]: string } = {};

    public bibliographyIds: Array<string> = [];
    public bibliography: { [id: string]: string } = {};

    public orderedClusterIds: Array<ClusterPosition> = [];

    /** For showing a paint splash when clusters are updated */
    public updatedLastRevision: { [id: number]: boolean } = {};

    constructor(clusters: Cluster[], oci: Array<ClusterPosition>, driver: Driver) {
        this[immerable] = true;
        this.orderedClusterIds = oci;
        for (let cluster of clusters) {
            this.builtClusters[cluster.id] = driver.builtCluster(cluster.id);
            // TODO: send note through a round trip and get it from builtCluster
        }
    }

    public update(summary: UpdateSummary, oci: Array<ClusterPosition>) {
        let neu = produce(this, draft => {
            draft.updatedLastRevision = {};
            draft.orderedClusterIds = oci;
            let bib = summary.bibliography;
            if (bib != null) {
                let entry_ids = bib.entryIds;
                if (entry_ids != null) {
                    draft.bibliographyIds = entry_ids;
                }
                for (let key of Object.keys(bib.updatedEntries)) {
                    draft.bibliography[key] = bib.updatedEntries[key];
                }
            }
            for (let [id, built] of summary.clusters) {
                draft.builtClusters[id] = built;
                draft.updatedLastRevision[id] = true;
            }
        });
        return neu;
    }

}

class RefCounter {
    private citekeyRefcounts = new Map<string, number>();
    constructor(private constructor_: (string) => void, private destructor: (string) => void) {}
    increment(cluster: Cluster) {
        for (let cite of cluster.cites) {
            let old = this.citekeyRefcounts.get(cite.id);
            this.citekeyRefcounts.set(cite.id, (old || 0) + 1);
            if (old === undefined) {
                this.constructor_(cite.id);
            }
        }
    }
    decrement(cluster: Cluster) {
        for (let cite of cluster.cites) {
            let old = this.citekeyRefcounts.get(cite.id)
            if (old !== undefined && old > 0) {
                const neu = old - 1;
                if (neu === 0) {
                    this.citekeyRefcounts.delete(cite.id);
                    this.destructor(cite.id);
                } else {
                    this.citekeyRefcounts.set(cite.id, neu);
                }
            }
        }
    }
}

/**
 * A Document wraps the Driver API and stores its own copy of the cite clusters.
 * It keeps the clusters in sync, and also maintains an up-to-date copy of the
 * _rendered_ result. It interacts with the Driver through the update queue
 * rather than asking it to serialize every rendering on every change.
 *
 * It's easier (outside this class) to have an immutable value so that React
 * knows when things need re-rendering. For a larger document, this is more
 * performant, as long as you use React.memo or PureComponent in the right places.
 * */
export class Document {

    /** The brains of the operation */
    private driver: Driver;

    /** The internal document model */
    public clusters: Cluster[];

    public rendered: RenderedDocument;

    private refCounts: RefCounter;

    private nextClusterId = 100;
    private nextCiteId = 100;

    constructor(clusters: Cluster[], driver?: Driver) {
        this.refCounts = new RefCounter(
            key => {
                // console.log("reference subscribed", key);
            },
            key => {
                // Unsubscribe from changes in Zotero, etc.
                // console.log("reference destructor:", key);
            }
        );
        this.clusters = clusters;
        this.initCitekeys();
        if (driver) {
            this.init(driver);
        }
    }

    private initCitekeys() {
        for (let cluster of this.clusters) {
            this.refCounts.increment(cluster);
        }
    }

    private init(driver: Driver) {
        this.driver = driver;
        driver.initClusters(this.clusters);
        driver.setClusterOrder(this.clusterPositions());
        this.rendered = new RenderedDocument(this.clusters, this.clusterPositions(), driver);
        // Drain the update queue, because we know we're up to date
        this.driver.drain();
    }

    /** Warning: Does not free the old driver. You should have kept a copy to call free() on. */
    rebuild(withNewDriver: Driver): Document {
        this.init(withNewDriver);
        return this;
    }

    selfUpdate(): Document {
        return this.produce(() => {});
    }

    /** Immutably assemble a new document */
    produce(fn: (draft: Draft<Document>) => void) {
        let driver = this.driver;
        return produce(this, draft => {
            fn(draft);
            driver.setClusterOrder(draft.clusterPositions());
            let summary = driver.batchedUpdates();
            draft.rendered = draft.rendered.update(summary, this.clusterPositions());
        });
    };

    clusterPositions(): Array<ClusterPosition> {
        // Simple but good for a demo: one note number per cluster.
        return this.clusters.map((c, i) => ({ id: c.id, note: i + 1 }));
    }

    //////////////
    // Clusters //
    //////////////

    createCluster(cites: Cite[]): Cluster {
        return {
            id: this.nextClusterId++,
            cites: cites,
        };
    }

    replaceCluster(cluster: Cluster) {
        // Mutate
        let idx = this.clusters.findIndex(c => c.id === cluster.id);
        this.refCounts.increment(cluster);
        this.refCounts.decrement(this.clusters[idx]);
        this.clusters[idx] = cluster;
        // Inform the driver
        this.driver.insertCluster(cluster);
    }

    removeCluster(id: number) {
        // Mutate
        let idx = this.clusters.findIndex(c => c.id === id);
        this.refCounts.decrement(this.clusters[idx]);
        this.clusters.splice(idx, 1);
        // Inform the driver
        this.driver.removeCluster(id);
    }

    // TODO: be able to pick up a cluster and move it
    /**
     * Presumes that each noteNumber will have only one cluster associated with
     * it. In Zotero's Word plugin, if using a note style, each footnote may
     * have multiple if you manually create a footnote and add clusters to it.
     * So this button is like the Zotero add cite button; it conceptually
     * inserts a footnote with a single cluster. In the Word plugin, a manual
     * footnote could have been inserted anywhere, so the document's footnote
     * numbers should be read and updated here, before running this. And for
     * adding a cluster to an existing footnote, there should be another
     * version of this function, i.e. `addClusterToFootnote`.
     * 
     * Obviously for in-text styles, footnote numbers don't matter, but should
     * be maintained as one-to-one so that switching to a note style works.
     * 
     * TODO: API for asking the driver what kind of style it is.
     * TODO: maybe use beforeNumber instead of beforeCluster
     * 
     * @param _cluster      A createCluster() result.
     * @param beforeCluster The cluster ID to insert this before; `null` = at the end.
     */
    insertCluster(cluster: Cluster, beforeCluster: ClusterId | null) {
        let pos = beforeCluster === null ? -1 : this.clusters.findIndex(c => c.id === beforeCluster);
        if (pos !== -1) {
            let atPos = this.clusters[pos];
            this.refCounts.increment(cluster);
            this.refCounts.decrement(atPos);
            this.clusters.splice(pos, 0, cluster);
            this.driver.insertCluster(cluster);
        } else {
            this.clusters.push(cluster);
            this.driver.insertCluster(cluster);
        }
    }

}

function inc(x: number | [number, number]): number | [number, number] {
    if (Array.isArray(x)) {
        let [a, b] = x;
        return [a+1, b];
    } else {
        return x + 1;
    }
}
