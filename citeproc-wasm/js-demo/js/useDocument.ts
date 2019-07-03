import { useState, useRef } from 'react';
import { Result, Err, Ok, Option, Some, None } from 'safe-types';
import { Driver, Reference, Cluster, Lifecycle, UpdateSummary } from '../../pkg/citeproc_wasm';
import { Document } from './Document';

/**
 * This isolates the free-ing of old drivers.
 * JS garbage collection isn't technically part of the spec, so there is no way to call free() automatically.
 * This solution uses React's useState.
 **/
export const useDriver = (initial: Result<Driver, any>) => {
    const [old, setOld] = useState(null as Driver);
    const [driver, setDriver] = useState<Result<[Driver, Promise<void>], any>>(initial);
    const request = useRef(Promise.resolve(true));
    const update = (d: Result<Driver, any>) => {
        if (driver.is_ok()) {
            if (old) {
                old.free();
            }
            setOld(driver.unwrap());
        }
        setDriver(d);
    };
    return [driver, update] as [Result<Driver, any>, (res: Result<Driver, any>) => void];
};

/**
 * This keeps a Driver, some References, and a Document in sync, i.e.:
 *
 * * when you supply a new Driver, its references are set and locales fetched,
 *   the Document is reconfigured to use it, and any existing clusters are added
 * * when you set references (all or some), the Driver is informed, and the
 *   Document gets an update
 * 
 * You will typically want to update references if a user has edited them. This
 * makes sure to wait for fetchAll() when modifying references, as they might
 * have new locales. Any syncing of clusters back to the Driver is done by
 * Document.
 *
 * Again, you don't have to use React hooks/useState etc and the Rust-like `safe-types`.
 * An example that would work in an imperative app (e.g. without React's automatic updating) is below.
 */
export const useDocument = (initialDriver: Result<Driver, any>, initialReferences: Reference[], initialClusters: Cluster[]) => {
    const [references, setReferences] = useState(initialReferences);
    const [driver, setDriver] = useDriver(initialDriver);
    const [document, setDocument] = useState(None() as Option<Document>);
    const [inFlight, setInFlight] = useState(false);

    // Setting references might mean waiting for a new locale to be fetched. So they're async 'methods'.

    const flightFetcher = async (driv: Driver) => {
        setInFlight(true);
        try {
            await driv.fetchAll();
        } finally {
            setInFlight(false);
        }
    }

    const resetReferences = async (refs: Reference[]) => {
        setReferences(refs);
        if (driver.is_ok()) {
            let d = driver.unwrap();
            d.setReferences(refs);
            await d.fetchAll();
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
            driver.is_ok() && driver.unwrap().insertReference(ref);
        }
        setReferences(neu);
        if (driver.is_ok()) {
            let d = driver.unwrap();
            await flightFetcher(d);
            setDocument(document.map(doc => doc.selfUpdate()));
        }
    };

    const updateDriver = async (res: Result<Driver, any>): Promise<void> => {
        if (res.is_ok()) {
            let newDriver = res.unwrap();
            newDriver.setReferences(references);
            await flightFetcher(newDriver);
            // doc updated/created to use newDriver, after ref-setting & fetching
            let newDoc = document.match({
                Some: old => old.rebuild(newDriver),
                None: () => new Document(initialClusters, newDriver),
            });
            setDocument(Some(newDoc));
        }
        setDriver(res);
    };

    return {
        document,
        driver,
        updateDriver,
        setDocument,
        inFlight,
        resetReferences,
        updateReferences,
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
class _ExampleManager implements Lifecycle {
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
        let oldDriver = this.driver;
        // must be async imported to use
        // let newDriver = Driver.new(styleText, this);
        let newDriver = undefined as Driver;
        newDriver.setReferences(this.references);
        newDriver.initClusters(this.clusters);
        // wait for any locales to come back
        await newDriver.fetchAll();
        this.driver = newDriver;
        if (oldDriver) {
            oldDriver.free();
        }
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
            this.driver.setReferences(refs);
            await this.driver.fetchAll();
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
            this.driver && this.driver.setReferences(refs);
        }
        this.references = neu;
        if (this.driver) {
            await this.driver.fetchAll();
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
        this.driver.replaceCluster(cluster);
        this.update();
    }
    // abstract etc(...args: any[]): void;

    private async getStyle(name: string): Promise<string> {
        // return await get(`/style?name=${name}.xml`);
        return "...";
    }

    async fetchLocale(lang: string): Promise<string> {
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
        const $: any = () => {};
        for (const [id, html] of summary.clusters) {
            // this only runs once per cluster that actually needed to be updated
            $("#cluster-" + id).innerHtml(html);
        }
    }
}
