import { Driver, UpdateSummary } from '@citeproc-rs/wasm';

const mkStyle = (inner: string) => {
    return `
    <style class="note">
      <citation>
        <layout>
          ${inner}
        </layout>
      </citation>
    </style>
    `;
}

const mkLocale = (lang: string, terms: { [key: string]: string }) => {
    return `
    <?xml version="1.0" encoding="utf-8"?>
    <locale xml:lang="${lang}">
    <terms>
        ${ Object.entries(terms).map((k,v) => `<term name="${k}">${v}</term>`).join("\n") }
    </terms>
    </locale>
    `;
}

class Fetcher {
    constructor(private callback: (lang: string) => void, private factory: (lang: string) => string) { }
    async fetchLocale(lang: string) {
        this.callback(lang);
        return this.factory(lang);
    }
}

const boringFetcher = new Fetcher(() => {}, (lang: string) => mkLocale(lang, {}));
const withDriver = (cfg: any, callback: (driver: Driver) => void) => {
    let style = cfg.style || mkStyle('<text variable="title" />');
    let fetcher = cfg.fetcher || boringFetcher;
    let fmt = cfg.format || "plain";
    let driver = Driver.new(style, fetcher, fmt);
    callback(driver);
    driver.free();
};
const oneOneOne = (driver: Driver, r?: any) => {
    let refr = {
        type: "book",
        title: "TEST_TITLE",
        ...r,
        id: "citekey"
    }
    driver.insertReference(refr);
    driver.initClusters([{id: 1, cites: [{id: "citekey"}]}]);
    driver.setClusterOrder([{ id: 1 }]);
};

test('boots', () => {
    withDriver({}, driver => {
        expect(driver).not.toBeNull();
    });
});

test('returns a single cluster, single cite, single ref', () => {
    withDriver({}, driver => {
        expect(driver).not.toBeNull();
        oneOneOne(driver);
        driver.insertReference({ id: "citekey", type: "book", title: "TEST_TITLE" });
        driver.initClusters([{id: 1, cites: [{id: "citekey"}]}]);
        driver.setClusterOrder([{ id: 1 }]);
        let res = driver.builtCluster(1);
        expect(res).toBe("TEST_TITLE");
    });
});

test('gets an update when ref changes', () => {
    withDriver({}, driver => {
        let updates: UpdateSummary;
        oneOneOne(driver);
        updates = driver.batchedUpdates();
        expect(updates.clusters).toContainEqual([1, "TEST_TITLE"]);
        driver.insertReference({ id: "citekey", type: "book", title: "TEST_TITLE_2" });
        updates = driver.batchedUpdates();
        expect(updates.clusters).toContainEqual([1, "TEST_TITLE_2"]);
    });
})
