import { Driver, UpdateSummary } from "@citeproc-rs/wasm";

export const mkNoteStyle = (inner: string, bibliography?: string) => {
    return `
    <style class="note">
      <info>
        <id>https://github.com/cormacrelf/citeproc-rs/test-style</id>
        <title>test-style</title>
        <updated>2000-01-01T00:00:00Z</updated>
      </info>
      <citation collapse="year">
        <layout>
          ${inner}
        </layout>
      </citation>
      ${ bibliography != null ? bibliography : "" }
    </style>
    `;
}

export const mkInTextStyle = (inner: string, bibliography?: string) => {
    return `
    <style class="in-text">
      <info>
        <id>https://github.com/cormacrelf/citeproc-rs/test-style</id>
        <title>test-style</title>
        <updated>2000-01-01T00:00:00Z</updated>
      </info>
      <citation collapse="year">
        <layout delimiter="; ">
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
    let style = cfg.style || mkNoteStyle('<text variable="title" />');
    let fetcher = cfg.fetcher || boringFetcher;
    let format = cfg.format || "plain";
    let driver = Driver.new({ style, fetcher, format }).unwrap();
    callback(driver);
    driver.free();
};

export const oneOneOne = (driver: Driver, r?: any, cid?: string) => {
    let refr = {
        type: "book",
        title: "TEST_TITLE",
        id: "citekey",
        ...r,
    };
    let id = refr.id;
    cid = cid || "one";
    driver.insertReference(refr).unwrap();
    driver.insertCluster({id: cid, cites: [{ id }]}).unwrap();
    driver.setClusterOrder([{ id: cid }]).unwrap();
};

export const checkUpdatesLen = (up: UpdateSummary, clusterCount: number, bibCount: number) => {
    let updates = up;
    expect(updates.clusters.length).toBe(clusterCount);
    let updatedKeys = Object.keys(updates.bibliography?.updatedEntries || {});
    expect(updatedKeys.length).toBe(bibCount);
};

