const { Driver } = require('../dist');

class Fetcher {
  async fetchLocale(lang) {
    let loc = '<?xml version="1.0" encoding="utf-8"?><locale xml:lang="' + lang + '"><terms><term name="edition">SUCCESS</term></terms></locale>';
    return loc;
  }
}

const initialClusters = [
  {
    id: 1,
    cites: [
      { id: "citekey" },
    ],
  },
  {
    id: 2,
    cites: [
      { id: "citekey" }
    ],
  },
  {
    id: 3,
    cites: [
      { id: "foreign", prefix: "Yeah" }
    ],
  },
];

let style = '<?xml version="1.0" encoding="utf-8"?>\n<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">\n<citation><layout><group delimiter=" "><text variable="title" /><text term="edition" form="long"/></group></layout></citation></style>';
// style = '<?xml version="1.0" encoding="utf-8"?>\n<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">\n<citation><zoink></zoink></citation></style>';

let prom = async () => {
  try {
    let fetcher = new Fetcher();
    let driver = new Driver(style, fetcher, "html");
    driver.insertReferences([
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
    driver.setClusterOrder([ {id: 1, note: 1}, {id: 2, note: 2}, {id: 3, note: 3} ])
    console.log(driver.toFetch());
    await driver.fetchLocales();
    let result = driver.builtCluster(3);
    console.log(result);
  } catch (e) {
    console.error(e);
  }
}

prom().then(() => {}).catch(() => {})

