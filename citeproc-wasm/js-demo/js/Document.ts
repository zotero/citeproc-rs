import { Reference, Cite, Cluster, Driver, UpdateSummary } from '../../pkg';
import { produce, immerable, Draft, IProduce } from 'immer';

export type ClusterId = number;
export type CiteId = number;

export class RenderedDocument {

    /** Caches HTML for a ClusterId, that is pulled from the driver */
    public builtClusters: { [id: number]: string } = {};

    public orderedClusterIds: Array<{ id: ClusterId; noteNumber: number; }> = [];

    /** For showing a paint splash when clusters are updated */
    public updatedLastRevision: { [id: number]: boolean } = {};

    constructor(clusters: Cluster[], driver: Driver) {
        this[immerable] = true;
        for (let cluster of clusters) {
            this.builtClusters[cluster.id] = stringifyInlines(driver.builtCluster(cluster.id));
            // TODO: send noteNumber through a round trip and get it from builtCluster
            this.orderedClusterIds.push({ id: cluster.id, noteNumber: cluster.noteNumber });
        }
    }

    public update(summary: UpdateSummary, oci: Array<{ id: ClusterId, noteNumber: number }>) {
        return produce(this, draft => {
            draft.updatedLastRevision = {};
            draft.orderedClusterIds = oci;
            for (let [id, built] of summary.clusters) {
                draft.builtClusters[id] = stringifyInlines(built);
                draft.updatedLastRevision[id] = true;
            }
        })
    }

}

type NonNumberedCluster = Omit<Cluster, "noteNumber">;
type UnidentifiedCite = Omit<Cite, "citeId">;

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

    private nextClusterId = 100;
    private nextCiteId = 100;

    constructor(clusters: Cluster[], driver: Driver) {
        this.clusters = clusters;
        this.init(driver);
    }

    private ordered() {
        return this.clusters.map(c => ({ id: c.id, noteNumber: c.noteNumber, }));
    }

    private init(driver: Driver) {
        this.driver = driver;
        driver.initClusters(this.clusters);
        this.rendered = new RenderedDocument(this.clusters, driver);
        // Drain the update queue, because we know we're up to date
        this.driver.batchedUpdates();
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
            let summary = driver.batchedUpdates();
            draft.rendered = draft.rendered.update(summary, this.ordered());
        });
    };

    //////////////
    // Clusters //
    //////////////

    replaceCluster(cluster: Cluster) {
        // Mutate
        let idx = this.clusters.findIndex(c => c.id === cluster.id);
        this.clusters[idx] = cluster;
        // Inform the driver
        this.driver.replaceCluster(cluster);
    }

    createCite(_cite: UnidentifiedCite): Cite {
        let cite = _cite as Cite;
        cite.citeId = this.nextCiteId++;
        return cite;
    }

    createCluster(cites: UnidentifiedCite[]): NonNumberedCluster {
        return {
            id: this.nextClusterId++,
            cites: cites.map(c => this.createCite(c))
        };
    }

    // TODO: be able to pick up a cluster and move it
    /**
     * @param _cluster A createCluster() result.
     * @param   before The cluster ID to insert this before; `null` = at the end.
     */
    insertCluster(_cluster: NonNumberedCluster, before: ClusterId | null) {
        let cluster = _cluster as Cluster;
        let pos = before === null ? -1 : this.clusters.findIndex(c => c.id === before);
        if (pos !== -1) {
            let atPos = this.clusters[pos];
            cluster.noteNumber = atPos.noteNumber;
            this.clusters.splice(pos, 0, cluster);
            let arr = [];
            // cascade to the rest of it;
            // modifies this.clusters at the same time as assembling an updater for the driver
            // e.g. [2, 3, 3, 4, 4, 5, 5, 6, ...]
            for (let i = pos + 1; i < this.clusters.length; i++) {
                let cl = this.clusters[i];
                arr.push(cl.id);
                arr.push(++cl.noteNumber);
            }
            // The invariant to uphold is that note numbers increase monotonically with the cluster order.
            // So you can have n1 and n2 not containing any clusters ie
            //     n1 -> []
            //     n2 -> []
            //     n3 -> [c1, c2]
            //     n4 -> [c3]
            // But you cannot have
            //     n1 -> [c2] 
            //     n2 -> [c1] 
            // Equivalently
            //     c1 = { id: ..., noteNumber: 2 }
            //     c2 = { id: ..., noteNumber: 1 }
            this.driver.insertCluster(cluster, before);
            this.driver.renumberClusters(new Uint32Array(arr))
        } else {
            cluster.noteNumber = this.clusters[this.clusters.length - 1].noteNumber + 1;
            this.clusters.push(cluster);
            this.driver.insertCluster(cluster, before);
        }
    }

}

// Pandoc JSON won't be the output format forever -- when Salsa can do
// generics, then we will produce preformatted HTML strings.
interface Str { t: "Str", c: string };
interface Span { t: "Span", c: [any, Inline[]] };
interface Emph { t: "Emph", c: Inline[] };
interface Strikeout { t: "Strikeout", c: Inline[] };
interface Space { t: "Space" };
type Inline = Str | Space | Span | Emph | Strikeout;
export function stringifyInlines(inlines: Inline[]): string {
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
