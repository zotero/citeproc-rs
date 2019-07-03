import { Cite, Cluster, Driver, UpdateSummary } from '../../pkg/citeproc_wasm';
import { produce, immerable } from 'immer';

export class Document {
    /** Caches HTML for a ClusterId */
    public builtClusters: { [id: number]: string } = {};
    public updatedLastRevision: { [id: number]: boolean } = {};

    constructor(public clusters: Cluster[], driver: Driver) {
        this[immerable] = true;
        for (let cluster of clusters) {
            this.builtClusters[cluster.id] = stringifyInlines(driver.builtCluster(cluster.id));
        }
        // Drain the update queue, because we know we're up to date
        driver.batchedUpdates();
    }

    /** Immutably updates the document to include all the Driver's batched updates in a summary.  */
    merge(summary: UpdateSummary): Document {
        console.info(summary);
        return produce(this, draft => {
            draft.updatedLastRevision = {};
            for (let [id, built] of summary.clusters) {
                draft.builtClusters[id] = stringifyInlines(built);
                draft.updatedLastRevision[id] = true;
            }
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
