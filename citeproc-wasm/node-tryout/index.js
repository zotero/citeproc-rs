const citeproc = require('citeproc-wasm');

let style = '<?xml version="1.0" encoding="utf-8"?>\n<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">\n<citation><layout></layout></citation></style>';
// style = '<?xml version="1.0" encoding="utf-8"?>\n<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">\n<citation><zoink></zoink></citation></style>';
try {
  let result = citeproc.parse(style);
  console.log(result);
} catch (e) {
  console.error(e);
}
