import { Driver, UpdateSummary } from "@citeproc-rs/wasm";

export const mkStyle = (inner: string, bibliography?: string) => {
    return `
    <style class="note">
      <citation>
        <layout>
          ${inner}
        </layout>
      </citation>
      ${ bibliography != null ? bibliography : "" }
    </style>
    `;
}

export const mkLocale = (lang: string, terms: { [key: string]: string }) => {
    return `
    <?xml version="1.0" encoding="utf-8"?> <locale xml:lang="${lang}">
    <terms>
        ${ Object.entries(terms).map((k,v) => `<term name="${k}">${v}</term>`).join("\n") }
    </terms>
    </locale>
    `;
}

export class Fetcher {
    constructor(private callback: (lang: string) => void, private factory: (lang: string) => string) { }
    async fetchLocale(lang: string) {
        this.callback(lang);
        return this.factory(lang);
    }
}

export const boringFetcher = new Fetcher(
    () => {},
    (lang: string) => mkLocale(lang, {})
);

export const withDriver = (cfg: any, callback: (driver: Driver) => void) => {
    let style = cfg.style || mkStyle('<text variable="title" />');
    let fetcher = cfg.fetcher || boringFetcher;
    let fmt = cfg.format || "plain";
    let driver = Driver.new(style, fetcher, fmt);
    callback(driver);
    driver.free();
};

export const oneOneOne = (driver: Driver, r?: any) => {
    let refr = {
        type: "book",
        title: "TEST_TITLE",
        ...r,
        id: "citekey"
    }
    driver.insertReference(refr);
    driver.insertCluster({id: "one", cites: [{id: "citekey"}]});
    driver.setClusterOrder([{ id: "one" }]);
};

export const checkUpdatesLen = (up: UpdateSummary, clusterCount: number, bibCount: number) => {
    let updates = up;
    expect(updates.clusters.length).toBe(clusterCount);
    let updatedKeys = Object.keys(updates.bibliography?.updatedEntries || {});
    expect(updatedKeys.length).toBe(bibCount);
};

