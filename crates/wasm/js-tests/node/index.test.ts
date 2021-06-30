import { withDriver, oneOneOne, mkNoteStyle, mkInTextStyle, checkUpdatesLen } from './utils';
import {UpdateSummary} from '@citeproc-rs/wasm';

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
            driver.initClusters([{id: "one", cites: [{id: "citekey"}]}]);
            driver.setClusterOrder([{ id: "one" }]);
            let res = driver.builtCluster("one").unwrap();
            expect(res).toBe("TEST_TITLE");
        });
    });
});

describe("batchedUpdates", () => {
    test('gets an update when ref changes', () => {
        withDriver({}, driver => {
            oneOneOne(driver);
            let updates = driver.batchedUpdates().unwrap();
            expect(updates.clusters).toContainEqual(["one", "TEST_TITLE"]);
            driver.insertReference({ id: "citekey", type: "book", title: "TEST_TITLE_2" });
            updates = driver.batchedUpdates().unwrap();
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
        withDriver({style: bibStyle}, driver => {
            oneOneOne(driver);
            let full = driver.fullRender().unwrap();
            expect(full.allClusters).toHaveProperty("one", "TEST_TITLE");
            expect(full.bibEntries).toContainEqual({ id: "citekey", value: "TEST_TITLE" });
        })
    });

    test('update queue generally', () => {
        withDriver({style: bibStyle}, driver => {
            let once: UpdateSummary, twice: UpdateSummary;

            // Initially empty
            checkUpdatesLen(driver.batchedUpdates().unwrap(), 0, 0);

            // Add stuff, check it creates
            oneOneOne(driver);
            checkUpdatesLen(driver.batchedUpdates().unwrap(), 1, 1);

            // Now fullRender one should drain the queue.
            driver.fullRender();
            checkUpdatesLen(driver.batchedUpdates().unwrap(), 0, 0);

            // Edit a reference
            oneOneOne(driver, { title: "ALTERED" });
            let up = driver.batchedUpdates().unwrap();
            checkUpdatesLen(up, 1, 1);
            expect(up.bibliography?.entryIds).toEqual(null);
            expect(driver.builtCluster("one").unwrap()).toBe("ALTERED");

            // Should have no updates, as we just called batchedUpdates.
            once = driver.batchedUpdates().unwrap(); twice = driver.batchedUpdates().unwrap();
            expect(once).toEqual(twice);
            expect(once.bibliography).toBeFalsy();
            checkUpdatesLen(once, 0, 0);

            // Only inserting a reference does nothing
            driver.insertReference({ id: "added", type: "book", title: "ADDED" });
            once = driver.batchedUpdates().unwrap(); twice = driver.batchedUpdates().unwrap();
            expect(once).toEqual(twice);

            // Only inserting a cluster not in the document does nothing
            driver.insertCluster({ id: "123", cites: [{ id: "added" }] }).unwrap();
            once = driver.batchedUpdates().unwrap(); twice = driver.batchedUpdates().unwrap();
            expect(once).toEqual(twice); checkUpdatesLen(once, 0, 0);

            // Add it to the document, and it's a different story
            driver.setClusterOrder([ {id:"one"},{id:"123"} ]).unwrap();
            once = driver.batchedUpdates().unwrap(); twice = driver.batchedUpdates().unwrap();
            expect(once).not.toEqual(twice); checkUpdatesLen(twice, 0, 0);
            expect(once.clusters).toContainEqual(["123", "ADDED"]);
            expect(once.bibliography?.entryIds).toEqual(["citekey", "added"]);
            expect(once.bibliography?.updatedEntries).toHaveProperty("added", "ADDED");
        })
    });

});

describe("previewCitationCluster", () => {

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

    function pccSetup(callback) {
        withDriver({ style: ibidStyle }, driver => {
            let one = "cluster-one";
            let two = "cluster-two";
            oneOneOne(driver, { title: "ONE", id: "r1" }, "cluster-one");
            oneOneOne(driver, { title: "TWO", id: "r2" }, "cluster-two");
            driver.setClusterOrder([{ id: one }, { id: two }]).unwrap();
            callback(driver, [one, two]);
        })
    }

    // There are more extensive tests already in rust, so this is more of a smoke test.
    test("between two other clusters", () => {
        pccSetup((driver, [one, two]) => {
            // between the other two
            let pcc = driver.previewCitationCluster(
                [ { id: "r1" } ],
                [{ id: one }, { }, { id: two }],
                "plain"
            ).unwrap();
            expect(pcc).toEqual("ibid");
        })
    })

    test("replacing a cluster", () => {
        pccSetup((driver, [one, two]) => {
            // replacing #1
            var pcc = driver.previewCitationCluster(
                [ { id: "r1" } ],
                [{ }, { id: two }],
                "plain"
            ).unwrap();
            expect(pcc).toEqual("ONE");
            // replacing #1, with note numbers isntead
            pcc = driver.previewCitationCluster(
                [ { id: "r1" } ],
                [{ note: 1, }, { id: two, note: 5 }],
                "plain"
            ).unwrap();
            expect(pcc).toEqual("ONE");
        })
    })

    test("should error when supplying unsupported output format",() => {
        pccSetup((driver, [one, two]) => {
            let res = driver.previewCitationCluster([{id: "r1"}], [{}], "plaintext");
            expect(() => res.unwrap()).toThrow("Unknown output format \"plaintext\"");
        })
    })

});

describe("author-only and friends", () => {
    let style = mkInTextStyle(
        `
        <group delimiter=", ">
            <names variable="author" />
            <text variable="title" />
        </group>
    `,
        ``,
        { class: "in-text" }
    );

    function withSupp(callback) {
        withDriver({ style }, driver => {
            let one = "cluster-one";
            let two = "cluster-two";
            oneOneOne(driver, { title: "ONE", id: "r1", author: [{family: "Smith"}] }, "cluster-one");
            oneOneOne(driver, { title: "TWO", id: "r2", author: [{family: "Jones"}] }, "cluster-two");
            driver.setClusterOrder([{ id: one }, { id: two }]).unwrap();
            callback(driver, [one, two]);
        })
    }

    describe("on a Cite", () => {
        test("should accept suppress-author: true", () => {
            withSupp((driver, [one, two]) => {
                driver.insertCluster({ id: one, cites: [{ id: "r1" }] }).unwrap();
                expect(driver.builtCluster(one).unwrap()).toEqual("Smith, ONE");

                driver.insertCluster({ id: one, cites: [{ id: "r1", "suppress-author": true }] }).unwrap();
                expect(driver.builtCluster(one).unwrap()).toEqual("ONE");
            })
        });
        test("should accept author-only: true", () => {
            withSupp((driver, [one, two]) => {
                driver.insertCluster({ id: one, cites: [{ id: "r1" }] }).unwrap();
                expect(driver.builtCluster(one).unwrap()).toEqual("Smith, ONE");

                driver.insertCluster({ id: one, cites: [{ id: "r1", "author-only": true }] }).unwrap();
                expect(driver.builtCluster(one).unwrap()).toEqual("Smith");
            })
        });
    });

    describe("on a Cluster", () => {
        test("should accept mode: suppress-author", () => {
            withSupp((driver, [one, two]) => {
                driver.insertCluster({ id: one, mode: "suppress-author", cites: [{ id: "r1"}, {id: "r2"}] }).unwrap();
                expect(driver.builtCluster(one).unwrap()).toEqual("ONE; Jones, TWO");
                driver.insertCluster({ id: one, mode: "suppress-author", suppressFirst: 2, cites: [{ id: "r1", }, { id: "r2" }] }).unwrap();
                expect(driver.builtCluster(one).unwrap()).toEqual("ONE; TWO");
            })
        });
        test("should accept mode: author-only", () => {
            withSupp((driver, [one, two]) => {
                driver.insertCluster({ id: one, mode: "author-only", cites: [{ id: "r1", }] }).unwrap();
                expect(driver.builtCluster(one).unwrap()).toEqual("Smith");
            })
        });
        test("should accept mode: composite", () => {
            withSupp((driver, [one, two]) => {
                driver.insertCluster({ id: one, mode: "composite", cites: [{ id: "r1", }] }).unwrap();
                expect(driver.builtCluster(one).unwrap()).toEqual("Smith ONE");
                driver.insertCluster({ id: one, mode: "composite", infix: ", whose book", cites: [{ id: "r1", }] }).unwrap();
                expect(driver.builtCluster(one).unwrap()).toEqual("Smith, whose book ONE");
            })
        });
    });
});
