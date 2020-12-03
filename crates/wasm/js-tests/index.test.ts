import { withDriver, oneOneOne, mkStyle, checkUpdatesLen } from './utils';
import {UpdateSummary} from '@citeproc-rs/wasm';

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

let bibStyle = mkStyle(
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

let ibidStyle = mkStyle(
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

// There are more extensive tests already in rust, so this is more of a smoke test.
test("previewCitationCluster", () => {
    withDriver({ style: ibidStyle }, driver => {
        let one = "cluster-one";
        let two = "cluster-two";
        oneOneOne(driver, { title: "ONE", id: "r1" }, "cluster-one");
        oneOneOne(driver, { title: "TWO", id: "r2" }, "cluster-two");
        driver.setClusterOrder([{ id: one }, { id: two }]).unwrap();
        // between the other two
        let pcc = driver.previewCitationCluster(
            [ { id: "r1" } ],
            [{ id: one }, { }, { id: two }],
            "plain"
        ).unwrap();
        expect(pcc).toEqual("ibid");
        // replacing #1
        pcc = driver.previewCitationCluster(
            [ { id: "r1" } ],
            [{ }, { id: two }],
            "plain"
        ).unwrap();
        // replacing #1, with note numbers isntead
        pcc = driver.previewCitationCluster(
            [ { id: "r1" } ],
            [{ note: 1, }, { id: two, note: 5 }],
            "plain"
        ).unwrap();
        expect(pcc).toEqual("ONE");
    })
})

