import { Reference } from "../../pkg";
let refs: Reference[] = [
    {
        id: 'citekey',
        type: 'book',
        author: [
            { given: "Kurt", family: "Camembert" },
            { given: "Amadeus", family: "Rossi" },
            { given: "Ignatius", family: "Irrelevant" }
        ],
        title: "Where The Vile Things Are",
        issued: { "raw": "1999-08-09" },
    },
    {
        id: 'citekey2',
        type: 'book',
        author: [
            { given: "Kurt", family: "Camembert" },
            { given: "Ariadne", family: "Rossi" },
            { given: "Ignatius", family: "Irrelevant" }
        ],
        title: "Where The Vile Things Are",
        issued: { "raw": "1999-08-09" },
    },
    {
        id: "ysuf1",
        type: "book",
        title: "NoAuthor",
        issued: { "raw": "2009-07-07" },
    },
    {
        id: "ysuf2",
        type: "book",
        title: "NoAuthor",
        issued: { "raw": "2009-11-01" },
    },
    {
        id: 'foreign',
        type: 'book',
        title: "Some other title",
        language: 'fr-FR',
    }
];

let date = 1992;
let dupe: Reference = {
    id: "r",
    type: 'article-journal',
    title: "Article",
    author: [{ given: "Alicia", family: "Jones" }],
    issued: { 'date-parts': [[date]] }
};
for (let i = 1; i <= 20; i++) {
    refs.push({
        ...dupe,
        id: 'r' + i,
        title: dupe.title + " " + i,
        issued: { "date-parts": [[date + i]] }
    })
}

export const initialReferences = refs;
