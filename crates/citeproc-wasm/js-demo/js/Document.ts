import { Reference, Cite, NoteCluster, Driver, UpdateSummary } from '../../pkg';
import { produce, immerable, Draft, IProduce } from 'immer';

export type ClusterId = number;
export type CiteId = number;
export type OrderedClusterIds = Pick<NoteCluster, "id" | "note">;

export class RenderedDocument {

    /** Caches HTML for a ClusterId, that is pulled from the driver */
    public builtClusters: { [id: number]: string } = {};

    public orderedClusterIds: Array<OrderedClusterIds> = [];

    /** For showing a paint splash when clusters are updated */
    public updatedLastRevision: { [id: number]: boolean } = {};

    constructor(clusters: NoteCluster[], driver: Driver) {
        this[immerable] = true;
        for (let cluster of clusters) {
            this.builtClusters[cluster.id] = stringifyInlines(driver.builtCluster(cluster.id));
            // TODO: send note through a round trip and get it from builtCluster
            this.orderedClusterIds.push({ id: cluster.id, note: cluster.note });
        }
    }

    public update(summary: UpdateSummary, oci: Array<OrderedClusterIds>) {
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

type NonNumberedCluster = Omit<NoteCluster, "note">;
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
    public clusters: NoteCluster[];

    public rendered: RenderedDocument;

    private nextClusterId = 100;
    private nextCiteId = 100;

    constructor(clusters: NoteCluster[], driver: Driver) {
        this.clusters = clusters;
        this.init(driver);
    }

    private ordered() {
        return this.clusters.map(c => ({ id: c.id, note: c.note, }));
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

    replaceCluster(cluster: NoteCluster) {
        // Mutate
        let idx = this.clusters.findIndex(c => c.id === cluster.id);
        this.clusters[idx] = cluster;
        // Inform the driver
        this.driver.insertCluster(cluster);
    }

    removeCluster(id: number) {
        // Mutate
        let idx = this.clusters.findIndex(c => c.id === id);
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
    insertCluster(_cluster: NonNumberedCluster, beforeCluster: ClusterId | null) {
        let cluster = _cluster as NoteCluster;
        let pos = beforeCluster === null ? -1 : this.clusters.findIndex(c => c.id === beforeCluster);
        if (pos !== -1) {
            let atPos = this.clusters[pos];
            cluster.note = atPos.note;
            this.clusters.splice(pos, 0, cluster);
            let arr = [];
            // cascade to the rest of it;
            // modifies this.clusters at the same time as assembling an updater for the driver
            // e.g. [2, 3, 3, 4, 4, 5, 5, 6, ...]
            for (let i = pos + 1; i < this.clusters.length; i++) {
                let cl = this.clusters[i];
                cl.note = inc(cl.note);
                arr.push([cl.id, { note: cl.note }]);
            }
            this.driver.insertCluster(cluster);
            console.log(arr);
            this.driver.renumberClusters(arr)
        } else {
            if (this.clusters.length > 0) {
                cluster.note = inc(this.clusters[this.clusters.length - 1].note);
            } else {
                cluster.note = 1;
            }
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

// Pandoc JSON won't be the output format forever -- when Salsa can do
// generics, then we will produce preformatted HTML strings.
interface Str { t: "Str", c: string };
interface Span { t: "Span", c: [any, Inline[]] };
interface Emph { t: "Emph", c: Inline[] };
interface Strikeout { t: "Strikeout", c: Inline[] };
interface Space { t: "Space" };
type Inline = Str | Space | Span | Emph | Strikeout;
export function stringifyInlinesPandoc(inlines: Inline[]): string {
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

export function stringifyInlines(inlines: any): string {
    return inlines
}
