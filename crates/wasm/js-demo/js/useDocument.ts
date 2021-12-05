import { useState, useEffect } from 'react';
import { Result, Err, Option, Some, None } from 'safe-types';
import { parseStyleMetadata, Driver, Reference, Cluster, UpdateSummary, StyleMeta, Fetcher } from '../../pkg';
import { Document } from './Document';
import { CdnFetcher } from './CdnFetcher';

// Global for caching.
const fetcher = new CdnFetcher();

/**
 * This keeps a Driver, a style, some References, and a Document in sync, i.e.:
 *
 * * when you supply a new Driver, its references are set and locales fetched,
 *   the Document is reconfigured to use it, and any existing clusters are added
 * * when you set references (all or some), the Driver is informed, and the
 *   Document gets an update
 * * Frees old drivers. JS garbage collection isn't technically part of the spec,
 *   so there is no way to call free() automatically. This solution uses React's
 *   `useEffect` hook.
 * 
 * You will typically want to update references if a user has edited them. This
 * makes sure to wait for fetchLocales() when modifying references, as they might
 * have new locales. Any syncing of clusters back to the Driver is done by
 * Document.
 *
 * Again, you don't have to use React hooks/useState etc and the Rust-like `safe-types`.
 * An example that would work in an imperative app (e.g. without React's automatic updating) is below.
 */
export const useDocument = (initialStyle: string, initialReferences: Reference[], initialClusters: Cluster[]) => {
    const [references, setReferences] = useState(initialReferences);
    const [document, setDocument] = useState(None() as Option<Document>);
    const [inFlight, setInFlight] = useState(false);
    const [driver, setDriver] = useState<Result<Driver, CiteprocRsError>>(Err(new CiteprocRsError("uninitialized")));
    const [style, setStyle] = useState(initialStyle);
    const [metadata, setMetadata] = useState<Option<StyleMeta>>(None);
    const [error, setError] = useState<Option<CiteprocRsError>>(None());

    const flightFetcher = async (driv: Driver) => {
        setInFlight(true);
        try {
            await driv.fetchLocales();
        } finally {
            setInFlight(false);
        }
    }

    const createDriver = async (style: string) => {
        let d: Result<Driver, any> = Result.from(() => {
            try {
                let meta = parseStyleMetadata(style);
                setMetadata(Some(meta));
                let d = new Driver({
                    style,
                    fetcher,
                    format: "html",
                    // localeOverride: "de-AT",
                });
                d.resetReferences(references);
                return d;
            } catch (e) {
                if (e instanceof CiteprocRsError) {
                    setError(Some(e));
                } else {
                    throw e;
                }
            }
        });
        if (d.is_ok()) {
            let newDriver = d.unwrap();
            newDriver.resetReferences(references);
            await flightFetcher(newDriver);
        }
        setDriver(d);
    };

    const updateStyle = async (style: string) => await driver.match({
        Ok: async driver => {
            try {
                let meta = parseStyleMetadata(style);
                if (meta.info.parent != null) {
                    console.log("this is a dependent style!");
                    console.log(meta);
                }
                setMetadata(Some(meta));
                driver.setStyle(style);
                setError(None());
            } catch (e) {
                console.error(e);
                console.log(e.data);
                setError(Some(e));
            }
            await flightFetcher(driver);
            setDocument(document.map(doc => doc.selfUpdate()));
        },
        Err: async () => {
            createDriver(style);
        }
    });


    useEffect(() => { updateStyle(style); }, [style]);

    useEffect(() => {
        setError(None());
        driver.tap(newDriver => {
            // doc updated/created to use newDriver, after ref-setting & fetching
            let newDoc = Some(document.match({
                Some: old => old.rebuild(newDriver),
                None: () => new Document(initialClusters, newDriver),
            }));
            setDocument(newDoc);
        });
        return function cleanup() {
            driver.map(d => d.free());
        }
    }, [driver]);

    // Setting references might mean waiting for a new locale to be fetched. So they're async 'methods'.

    const resetReferences = async (refs: Reference[]) => {
        setReferences(refs);
        if (driver.is_ok()) {
            let d = driver.unwrap();
            d.resetReferences(refs);
            await flightFetcher(d);
            setDocument(document.map(doc => doc.selfUpdate()));
        }
    };

    const updateReferences = async (refs: Reference[]) => {
        let neu = references.slice(0);
        for (const ref of refs) {
            let i = neu.findIndex(r => r.id == ref.id);
            if (i === -1) {
                neu.push(ref);
            } else {
                neu[i] = ref;
            }
        }
        driver.tap(d => d.insertReferences(refs));
        setReferences(neu);
        if (driver.is_ok()) {
            let d = driver.unwrap();
            await flightFetcher(d);
            setDocument(document.map(doc => doc.selfUpdate()));
        }
    };


    return {
        document,
        // could be Ok(driver) but setStyle sets error to Err(e), then we want that error
        driver: error.into_result_err().and_then(() => driver),
        style,
        setStyle,
        setDocument,
        inFlight,
        resetReferences,
        updateReferences,
        references,
        metadata,
    };
}

/** 
 * An imperative example.
 * 
 * This example is a bit incomplete, because in an imperative app, Document is
 * not suitable as it hides the cool minimal update optimisation by turning
 * batched update instructions from the processor into a new immutable object
 * that loses this information. You probably want to:
 * 
 * * reimplement Document in this class (see example below)
 * * Implement the DocumentUpdater interface, and convert UpdateSummary directly
 *   into imperative display updates (or serialize some IPC messages to a word
 *   processing plugin, etc).
 * * Pass one of these to _ExampleManager
 */
class _ExampleManager implements Fetcher {
    private driver: Driver;

    /** You have some references and some cite clusters from a document. Let's start formatting citations. */
    constructor(private references: Reference[], private clusters: Cluster[], public handler: DocumentUpdater) {
    }

    private update() {
        if (this.driver) {
            let summary = this.driver.batchedUpdates();
            this.handler.handleUpdate(summary);
        }
    }

    /** Your user picked a new style from the list you know you can retrieve for them. */
    async setStyle(name: string) {
        let styleText = await this.getStyle(name);
        this.driver.setStyle(styleText);
        await this.driver.fetchLocales();
        this.update();
    }

    /** 
 * Call this when you no longer need the manager, because the database will
 * not be automatically garbage collected.
 */
    freeDriver() {
        this.driver.free();
        this.driver = undefined;
    }

    /**
 * You have just fetched the initial 200-odd references from your reference
 * manager. Use this to set the whole lot.
 */
    async resetReferences(refs: Reference[]) {
        this.references = refs;
        if (this.driver) {
            this.driver.resetReferences(refs);
            await this.driver.fetchLocales();
            this.update();
        }
    }

    /**
 * Your user's reference manager sends you a small part of your collection that got updated.
 * Use this instead of simply merging and running resetReferences. It will likely be faster.
 * 
 */
    async updateReferences(refs: Reference[]) {
        let neu = this.references.slice(0);
        for (const ref of refs) {
            let i = neu.findIndex(r => r.id == ref.id);
            // TODO: you probably also want to support deleting refs.
            // So a { upsert: [...], delete: [...] } structure, or multiple methods.
            if (i === -1) {
                neu.push(ref);
            } else {
                neu[i] = ref;
            }
            this.driver && this.driver.resetReferences(refs);
        }
        this.references = neu;
        if (this.driver) {
            await this.driver.fetchLocales();
            this.update();
        }
    }

    /**
     * Your user has modified a cite cluster. You want this reflected in the document.
     * Returns the updates you will need to make to the visible document to get it up to date.
     */
    replaceCluster(cluster: Cluster) {
        // see Document
        // this.clusters[...] = ...;
        // ...
        this.driver.insertCluster(cluster);
        this.update();
    }

    private async getStyle(_name: string): Promise<string> {
        // return await get(`/style?name=${name}.xml`);
        return "...";
    }

    async fetchLocale(_lang: string): Promise<string> {
        // You may want to include a cache if you switch style regularly.
        // return await get(`/locale-${name}.xml`);
        return "...";
    }
}

interface DocumentUpdater {
    handleUpdate(summary: UpdateSummary): void;
}

class DomUpdater implements DocumentUpdater {
    handleUpdate(summary: UpdateSummary): void {
        // fake jQuery
        const $: any = () => { };
        for (const [id, html] of summary.clusters) {
            // this only runs once per cluster that actually needed to be updated
            $("#cluster-" + id).innerHtml(html);
        }
    }
}
