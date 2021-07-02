#!/usr/bin/env node

const CSL = require("citeproc");
const yaml = require('js-yaml');
const fs = require("fs");
const path = require("path");
const program = require("commander");
const Sys = require("citeproc-test-runner/lib/sys");
const { styleCapabilities } = require("citeproc-test-runner/lib/style-capabilities");
const { parseFixture } = require('citeproc-test-runner/lib/fixture-parser');


program
  .version('0.0.1')
  .command('run <test_case.yml>')
  .action(run)

program
  .version('0.0.1')
  .command('to-yml <test_case.txt>')
  .action(to_yml)

// program
//   .version('0.0.1')
//   .command('to-txt <test_case.yml>')
//   .action((test_case) => { })

program.parse(process.argv);

function run(testCase) {
  var parsed = yaml.safeLoad(fs.readFileSync(testCase, 'utf8'));

  let library = {};
  for (let item of parsed.input) {
    library[item.id] = item;
  }

  class MySys extends Sys {
    retrieveItem(id) {
      return library[id];
    }
    retrieveLocale(loc) {
      return fs.readFileSync(path.join(
        process.env["HOME"],
          "Library",
          "Caches",
          "net.cormacrelf.citeproc-rs",
          "locales",
          "locales-" + loc + ".xml"
        ),
        'utf8'
      );
    }
  }

  let config = {
    styleCapabilities: styleCapabilities(parsed.csl),
  };
  let test = {
    NAME: testCase,
    OPTIONS: {},
    MODE: parsed.mode,
    INPUT: parsed.input,
    CSL: parsed.csl,
    CITATIONS: parsed['process-citation-clusters'],
    'CITATION-ITEMS': parsed.clusters && parsed.clusters.map(clust => {
      if (clust.cites) {
        if (clust.mode === 'composite') {
            console.warn("cluster has mode that will be thrown out:", clust.mode);
        }
        if (clust.mode === 'suppress-author' || clust.mode === 'author-only') {
            console.warn("cluster has mode that will be applied to every cite instead:", clust.mode);
            return clust.cites.map(cite => ({ [clust.mode]: true, ...cite }));
        }
        // under `- cites:`
        return clust.cites;
      }
      // plain vec of cites
      return clust;
    }) || undefined,
  };
  let logger_queue = [];

  const sys = new MySys(config, test, logger_queue);

  // console.log(sys.retrieveLocale("en-US"));
  // console.log(sys.retrieveItem("ITEM-1"));

  // let engine = new CSL.Engine(sys, parsed.csl);
  // let idList = parsed.input.map(i => i.id)
  // let citeItems = parsed.input.map(i => ({ id: i.id }));
  // engine.updateItems(idList);
  let res = sys.run();
  if (logger_queue.length > 0) console.debug(logger_queue);
  console.log(res);

  // console.log(yaml.safeDump(parsed));
}


function to_yml(txtFile) {
  let x = parseFixture({}, {}, txtFile);
  let y = {
    mode: x.MODE,
    result: x.RESULT,
    input: x.INPUT,
    'process-citation-clusters': x.CITATIONS && x.CITATIONS.map(([cluster, pre, post]) => ({ cluster, pre, post })),
    clusters: x['CITATION-ITEMS'] && x['CITATION-ITEMS'].map(cites => ({ cites })),
    'bib-entries': x['BIBENTRIES'],
    'bib-section': x['BIBSECTION'],
    csl: x.CSL,
  };
  for (let k of Object.keys(y)) {
    if (y[k] == null && k !== "result") {
      delete y[k];
    }
  }
  console.log(yaml.safeDump(y));
}


