import { Fetcher } from '../../pkg';

function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

export class CdnFetcher implements Fetcher {
    private cache: { [lang: string]: string } = {};
    async fetchLocale(lang: string) {
        if (typeof this.cache[lang] === 'string') {
            return this.cache[lang];
        }
        // this works
        // console.log(lang, "sleeping");
        // await sleep(400);
        // console.log(lang, "waking");
        let res = await fetch(`https://cdn.rawgit.com/citation-style-language/locales/master/locales-${lang}.xml`);
        if (res.ok) {
            let text = await res.text();
            this.cache[lang] = text;
            return text;
        }
    }
}
