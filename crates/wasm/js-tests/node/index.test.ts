import { withDriver, oneOneOne, mkNoteStyle, mkInTextStyle, checkUpdatesLen } from './utils';
import { UpdateSummary, Driver } from '@citeproc-rs/wasm';

let italicStyle = mkNoteStyle(
    `
        <text variable="title" font-style="italic" />
        <text variable="URL" prefix=" " />
    `,
);
let boldStyle = mkNoteStyle(
    `
        <text variable="title" font-weight="bold" />
        <text variable="URL" prefix=" " />
    `,
);
let authorTitleStyle = mkInTextStyle(
    `
    <group delimiter=", ">
    <names variable="author" />
    <text variable="title" />
    </group>
    `,
    ``
);


describe("Driver", () => {

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
            driver.initClusters([{ id: "one", cites: [{ id: "citekey" }] }]);
            driver.setClusterOrder([{ id: "one" }]);
            let res = driver.builtCluster("one");
            expect(res).toBe("TEST_TITLE");
        });
    });


    test("can setOutputFormat", () => {
        withDriver({ style: italicStyle, format: "html" }, driver => {
            const one = "one";
            oneOneOne(driver, { title: "Italicised", URL: "https://google.com" }, one);
            expect(driver.builtCluster(one))
                .toBe("<i>Italicised</i> <a href=\"https://google.com/\">https://google.com</a>");
            driver.setOutputFormat("html", {});
            expect(driver.builtCluster(one))
                .toBe("<i>Italicised</i> <a href=\"https://google.com/\">https://google.com</a>");
            driver.setOutputFormat("html", { linkAnchors: false });
            expect(driver.builtCluster(one))
                .toBe("<i>Italicised</i> https://google.com");
            driver.setOutputFormat("rtf", { linkAnchors: false });
            expect(driver.builtCluster(one))
                .toBe("{\\i Italicised} https://google.com");
            driver.setOutputFormat("plain");
            expect(driver.builtCluster(one))
                .toBe("Italicised https://google.com");
        })
    });
});

describe("batchedUpdates", () => {
    test('gets an update when ref changes', () => {
        withDriver({}, driver => {
            oneOneOne(driver);
            let updates = driver.batchedUpdates();
            expect(updates.clusters).toContainEqual(["one", "TEST_TITLE"]);
            driver.insertReference({ id: "citekey", type: "book", title: "TEST_TITLE_2" });
            updates = driver.batchedUpdates();
            expect(updates.clusters).toContainEqual(["one", "TEST_TITLE_2"]);
        });
    });

    let bibStyle = mkNoteStyle(
        '<text variable="title" />',
        `<bibliography>
            <layout>
                <text variable = "title" />
            </layout>
        </bibliography>`
    );

    test('fullRender works', () => {
        withDriver({ style: bibStyle }, driver => {
            oneOneOne(driver);
            let full = driver.fullRender();
            expect(full.allClusters).toHaveProperty("one", "TEST_TITLE");
            expect(full.bibEntries).toContainEqual({ id: "citekey", value: "TEST_TITLE" });
        })
    });

    test('update queue generally', () => {
        withDriver({ style: bibStyle }, driver => {
            let once: UpdateSummary, twice: UpdateSummary;

            // Initially empty
            checkUpdatesLen(driver.batchedUpdates(), 0, 0);

            // Add stuff, check it creates
            oneOneOne(driver);
            checkUpdatesLen(driver.batchedUpdates(), 1, 1);

            // Now fullRender one should drain the queue.
            driver.fullRender();
            checkUpdatesLen(driver.batchedUpdates(), 0, 0);

            // Edit a reference
            oneOneOne(driver, { title: "ALTERED" });
            let up = driver.batchedUpdates();
            checkUpdatesLen(up, 1, 1);
            expect(up.bibliography?.entryIds).toEqual(null);
            expect(driver.builtCluster("one")).toBe("ALTERED");

            // Should have no updates, as we just called batchedUpdates.
            once = driver.batchedUpdates(); twice = driver.batchedUpdates();
            expect(once).toEqual(twice);
            expect(once.bibliography).toBeFalsy();
            checkUpdatesLen(once, 0, 0);

            // Only inserting a reference does nothing
            driver.insertReference({ id: "added", type: "book", title: "ADDED" });
            once = driver.batchedUpdates(); twice = driver.batchedUpdates();
            expect(once).toEqual(twice);

            // Only inserting a cluster not in the document does nothing
            driver.insertCluster({ id: "123", cites: [{ id: "added" }] });
            once = driver.batchedUpdates(); twice = driver.batchedUpdates();
            expect(once).toEqual(twice); checkUpdatesLen(once, 0, 0);

            // Add it to the document, and it's a different story
            driver.setClusterOrder([{ id: "one" }, { id: "123" }]);
            once = driver.batchedUpdates(); twice = driver.batchedUpdates();
            expect(once).not.toEqual(twice); checkUpdatesLen(twice, 0, 0);
            expect(once.clusters).toContainEqual(["123", "ADDED"]);
            expect(once.bibliography?.entryIds).toEqual(["citekey", "added"]);
            expect(once.bibliography?.updatedEntries).toHaveProperty("added", "ADDED");
        })
    });

    test("produces updates when style or output format change", () => {
        withDriver({ style: italicStyle, format: "html" }, driver => {
            const one = "one";
            let once: UpdateSummary, twice: UpdateSummary;
            oneOneOne(driver, { title: "Italicised", URL: "https://a.com" }, one);

            expect(driver.builtCluster(one)).toBe('<i>Italicised</i> <a href="https://a.com/">https://a.com</a>');
            driver.batchedUpdates();

            // no change
            driver.setOutputFormat("html", {});
            once = driver.batchedUpdates(); twice = driver.batchedUpdates();
            expect(once).toEqual(twice);
            checkUpdatesLen(once, 0, 0); checkUpdatesLen(twice, 0, 0);

            // change part of FormatOptions
            driver.setOutputFormat("html", { linkAnchors: false });
            once = driver.batchedUpdates(); twice = driver.batchedUpdates();
            expect(once).not.toEqual(twice);
            expect(once.clusters).toContainEqual([one, '<i>Italicised</i> https://a.com']);
            checkUpdatesLen(twice, 0, 0);

            // change to rtf
            driver.setOutputFormat("rtf", { linkAnchors: false });
            once = driver.batchedUpdates(); twice = driver.batchedUpdates();
            expect(once).not.toEqual(twice);
            expect(once.clusters).toContainEqual([one, '{\\i Italicised} https://a.com']);
            checkUpdatesLen(twice, 0, 0);

            // change style to bold instead of italic
            driver.setStyle(boldStyle);
            once = driver.batchedUpdates(); twice = driver.batchedUpdates();
            expect(once).not.toEqual(twice);
            expect(once.clusters).toContainEqual([one, '{\\b Italicised} https://a.com']);
            checkUpdatesLen(twice, 0, 0);

            // back to html
            driver.setOutputFormat("html", { linkAnchors: false });
            once = driver.batchedUpdates(); twice = driver.batchedUpdates();
            expect(once).not.toEqual(twice);
            expect(once.clusters).toContainEqual([one, "<b>Italicised</b> https://a.com"]);
            checkUpdatesLen(twice, 0, 0);
        })
    });

});

describe("previewCluster", () => {

    let ibidStyle = mkNoteStyle(
        `
        <choose>
            <if position="ibid">
                <text value="ibid" />
            </if>
            <else>
                <text variable="title" />
            </else>
        </choose>
        `,
        ``,
    );

    function pccSetup(callback: (driver: Driver, ids: [string, string]) => void) {
        withDriver({ style: ibidStyle }, driver => {
            let one = "cluster-one";
            let two = "cluster-two";
            oneOneOne(driver, { title: "ONE", id: "r1" }, "cluster-one");
            oneOneOne(driver, { title: "TWO", id: "r2" }, "cluster-two");
            driver.setClusterOrder([{ id: one }, { id: two }]);
            callback(driver, [one, two]);
        })
    }

    // There are more extensive tests already in rust, so this is more of a smoke test.
    test("between two other clusters", () => {
        pccSetup((driver, [one, two]) => {
            // between the other two
            let pcc = driver.previewCluster(
                { cites: [{ id: "r1" }] },
                [{ id: one }, {}, { id: two }],
                "plain"
            );
            expect(pcc).toEqual("ibid");
        })
    })

    test("replacing a cluster", () => {
        pccSetup((driver, [_, two]) => {
            // replacing #1
            var pcc = driver.previewCluster(
                { cites: [{ id: "r1" }] },
                [{}, { id: two }],
                "plain"
            );
            expect(pcc).toEqual("ONE");
            // replacing #1, with note numbers isntead
            pcc = driver.previewCluster(
                { cites: [{ id: "r1" }] },
                [{ note: 1, }, { id: two, note: 5 }],
                "plain"
            );
            expect(pcc).toEqual("ONE");
        })
    })

    test("should error when supplying unsupported output format", () => {
        pccSetup((driver) => {
            let closure = () => driver.previewCluster({ cites: [{ id: "r1" }] }, [{}], "plaintext");
            expect(closure).toThrow(/Unknown output format \"plaintext\"/);
        })
    });

    test("should allow omitting the format argument", () => {
        pccSetup((driver, [_, two]) => {
            let res = driver.previewCluster(
                { cites: [{ id: "r1" }] },
                [{ note: 1 }, { id: two, note: 5 }]
            );
            expect(res).toEqual("ONE");
        })
    });

    test("should handle cluster modes", () => {
        pccSetup((driver, [_, two]) => {
            driver.setStyle(authorTitleStyle);
            driver.insertReference(
                { title: "ONE", id: "r1", type: "book", author: [{ family: "Smith" }] }
            );
            let res = driver.previewCluster(
                { cites: [{ id: "r1" }], mode: "Composite", infix: ", whose book" },
                [{ note: 1 }, { id: two, note: 5 }]
            );
            expect(res).toEqual("Smith, whose book ONE");
        })
    });

    test("should also work via deprecated previewCitationCluster(cites: Cite[], ...)", () => {
        pccSetup((driver, [_, two]) => {
            let res = driver.previewCitationCluster(
                [{ id: "r1" }],
                [{ note: 1 }, { id: two, note: 5 }]
            );
            expect(res).toEqual("ONE");
        })
    });

});

describe("AuthorOnly and friends", () => {
    function withSupp(callback: (driver: Driver, ids: [string, string]) => void) {
        withDriver({ style: authorTitleStyle }, driver => {
            let one = "cluster-one";
            let two = "cluster-two";
            oneOneOne(driver, { title: "ONE", id: "r1", author: [{ family: "Smith" }] }, "cluster-one");
            oneOneOne(driver, { title: "TWO", id: "r2", author: [{ family: "Jones" }] }, "cluster-two");
            driver.setClusterOrder([{ id: one }, { id: two }]);
            callback(driver, [one, two]);
        })
    }

    describe("on a Cite", () => {
        test("should accept mode: SuppressAuthor", () => {
            withSupp((driver, [one, _]) => {
                driver.insertCluster({ id: one, cites: [{ id: "r1" }] });
                expect(driver.builtCluster(one)).toEqual("Smith, ONE");

                driver.insertCluster({ id: one, cites: [{ id: "r1", mode: "SuppressAuthor" }] });
                expect(driver.builtCluster(one)).toEqual("ONE");
            })
        });
        test("should accept mode: AuthorOnly", () => {
            withSupp((driver, [one, _]) => {
                driver.insertCluster({ id: one, cites: [{ id: "r1" }] });
                expect(driver.builtCluster(one)).toEqual("Smith, ONE");

                driver.insertCluster({ id: one, cites: [{ id: "r1", mode: "AuthorOnly" }] });
                expect(driver.builtCluster(one)).toEqual("Smith");
            })
        });
    });

    describe("on a Cluster", () => {
        test("should accept mode: SuppressAuthor", () => {
            withSupp((driver, [one, _]) => {
                driver.insertCluster({ id: one, mode: "SuppressAuthor", cites: [{ id: "r1" }, { id: "r2" }] });
                expect(driver.builtCluster(one)).toEqual("ONE; Jones, TWO");
                driver.insertCluster({ id: one, mode: "SuppressAuthor", suppressFirst: 2, cites: [{ id: "r1", }, { id: "r2" }] });
                expect(driver.builtCluster(one)).toEqual("ONE; TWO");
            })
        });
        test("should accept mode: AuthorOnly", () => {
            withSupp((driver, [one, _]) => {
                driver.insertCluster({ id: one, mode: "AuthorOnly", cites: [{ id: "r1", }] });
                expect(driver.builtCluster(one)).toEqual("Smith");
            })
        });
        test("should accept mode: Composite", () => {
            withSupp((driver, [one, _]) => {
                driver.insertCluster({ id: one, mode: "Composite", cites: [{ id: "r1", }] });
                expect(driver.builtCluster(one)).toEqual("Smith ONE");
                driver.insertCluster({ id: one, mode: "Composite", infix: ", whose book", cites: [{ id: "r1", }] });
                expect(driver.builtCluster(one)).toEqual("Smith, whose book ONE");
                driver.insertCluster({ id: one, mode: "Composite", infix: ", whose book", suppressFirst: 0, cites: [{ id: "r1", }] });
                expect(driver.builtCluster(one)).toEqual("Smith, whose book ONE");
            })
        });
    });

    let styleWithIntext = mkInTextStyle(
        `
        <group delimiter=", ">
            <names variable="author" />
            <text variable="title" />
        </group>
    `,
        `<intext><layout>
            <text value="intext element" />
        </layout></intext>`
    );

    describe("<intext> element", () => {
        test("should parse when custom-intext feature is enabled", () => {
            withDriver({ style: styleWithIntext, cslFeatures: ["custom-intext"] }, driver => {
                let one = "cluster-one";
                driver.insertReference({ id: "r1", type: "book", title: "hi", })
                driver.insertCluster({ id: one, mode: "AuthorOnly", cites: [{ id: "r1", }] });
                driver.setClusterOrder([{ id: one }]);
                expect(driver.builtCluster(one)).toBe("intext element");
            });
        });
        test("should fail to parse when custom-intext feature is not enabled", () => {
            let closure = () => new Driver({ style: styleWithIntext, cslFeatures: [] });
            expect(closure).toThrowError(/Unknown element <intext>/);
        });
    });
});

describe("initialiser", () => {
    let urlStyle = mkInTextStyle(
        `
        <group delimiter=", ">
            <text variable="url" />
        </group>
        `,
    );

    test("enables linkAnchors by default", () => {
        withDriver({ style: urlStyle, format: "html" }, driver => {
            let one = "cluster-one";
            oneOneOne(driver, { title: "ONE", id: "r1", url: "https://example.com/nice?work=buddy&q=5" }, one);
            driver.setClusterOrder([{ id: one }]);
            expect(driver.builtCluster(one)).toEqual(`<a href="https://example.com/nice?work=buddy&q=5">https://example.com/nice?work=buddy&amp;q=5</a>`)
        });
    });
    test("can disable linkAnchors", () => {
        withDriver({ style: urlStyle, format: "html", formatOptions: { linkAnchors: false } }, driver => {
            let one = "cluster-one";
            oneOneOne(driver, { title: "ONE", id: "r1", url: "https://example.com/nice?work=buddy&q=5" }, one);
            driver.setClusterOrder([{ id: one }]);
            expect(driver.builtCluster(one)).toEqual(`https://example.com/nice?work=buddy&amp;q=5`)
        });
    });
});

