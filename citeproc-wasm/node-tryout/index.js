const { Driver } = require('../pkg');

class Fetcher {
  async fetchLocale(lang) {
    console.log("did fetch lang: ", lang)
    let loc = '<?xml version="1.0" encoding="utf-8"?><locale xml:lang="' + lang + '"><terms><term name="edition">SUCCESS</term></terms></locale>';
    console.log(loc);
    return loc;
  }
}

const initialClusters = [
  {
    id: 1,
    cites: [
      { id: 1, ref_id: "foreign" },
    ],
    note_number: 1,
  },
  {
    id: 2,
    cites: [
      { id: 2, ref_id: "citekey" }
    ],
    note_number: 2,
  },
  {
    id: 3,
    cites: [
      { id: 3, prefix: [{"t": "Str", "c": "Yeah, "}], ref_id: "foreign" }
    ],
    note_number: 3,
  },
];

let style = '<?xml version="1.0" encoding="utf-8"?>\n<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">\n<citation><layout><group delimiter=" "><text variable="title" /><text term="edition" form="long"/></group></layout></citation></style>';
// style = '<?xml version="1.0" encoding="utf-8"?>\n<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">\n<citation><zoink></zoink></citation></style>';

let prom = async () => {
  try {
    let fetcher = new Fetcher();
    let driver = Driver.new(style, fetcher);
    driver.setReferences([
      {
        id: 'citekey',
        type: 'book',
        author: [{given: "Kurt", family: "Camembert"}],
        title: "Where The Vile Things Are",
        issued: { "raw": "1999-08-09" },
        language: 'fr-FR',
      },
      {
        id: 'foreign',
        type: 'book',
        title: "Le Petit Bouchon",
        language: 'fr-FR',
      }
    ]);
    driver.initClusters(initialClusters);
    console.log(driver.toFetch());
    await driver.fetchAll();
    let result = driver.builtCluster(3);
    console.log(result);
  } catch (e) {
    console.error(e);
  }
}

prom().then(() => {}).catch(() => {})

