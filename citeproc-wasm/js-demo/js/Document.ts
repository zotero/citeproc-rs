import { Reference, Cite, Cluster, Driver, UpdateSummary } from '../../pkg/citeproc_wasm';
import { produce, immerable, Draft } from 'immer';

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

    /** Caches HTML for a ClusterId, that is pulled from the driver */
    public builtClusters: { [id: number]: string } = {};
    /** For showing a paint splash when clusters are updated */
    public updatedLastRevision: { [id: number]: boolean } = {};

    constructor(clusters: Cluster[], driver: Driver) {
        this[immerable] = true;
        this.clusters = clusters;
        this.init(driver);
    }

    private init(driver: Driver) {
        this.driver = driver;
        driver.initClusters(this.clusters);
        for (let cluster of this.clusters) {
            this.builtClusters[cluster.id] = stringifyInlines(driver.builtCluster(cluster.id));
        }
        // Drain the update queue, because we know we're up to date
        this.driver.batchedUpdates();
    }

    /** Warning: Does not free the old driver. You should have kept a copy to call free() on. */
    rebuild(withNewDriver: Driver) {
        return produce(this, draft => {
            // can't use methods directly on an immer draft
            Document.prototype.init.call(draft, withNewDriver);
        });
    }

    //////////////
    // Clusters //
    //////////////

    /** Immutably updates the document to include all the Driver's batched updates in a summary.  */
    private merge(summary: UpdateSummary): Document {
        return produce(this, draft => {
            Document.prototype.update.call(draft, summary);
        });
    }

    private update(summary: UpdateSummary) {
        this.updatedLastRevision = {};
        for (let [id, built] of summary.clusters) {
            this.builtClusters[id] = stringifyInlines(built);
            this.updatedLastRevision[id] = true;
        }
    }

    private static updateDraft(draft: Draft<Document>, driver: Driver): void {
        let summary = driver.batchedUpdates();
        Document.prototype.update.call(draft, summary);
    }

    selfUpdate() {
        return produce(this, draft => {
            Document.updateDraft(draft, this.driver);
        });
    }

    replaceCluster(cluster: Cluster): Document {
        let driver = this.driver;
        return produce(this, draft => {
            // Mutate
            let idx = draft.clusters.findIndex(c => c.id === cluster.id);
            draft.clusters[idx] = cluster;
            // Inform the driver
            driver.replaceCluster(cluster);
            // Pull queued updates
            Document.updateDraft(draft, driver);
        });
    }

    insertCluster(cluster: Cluster, ) {
        let driver = this.driver;
        return produce(this, draft => {
            // Mutate
            draft.clusters.push(cluster)
            // Inform the driver
            driver.insertCluster(cluster, null);
            // Pull queued updates
            Document.updateDraft(draft, driver);
        });
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
