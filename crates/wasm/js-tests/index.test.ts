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
        driver.initClusters([{id: 1, cites: [{id: "citekey"}]}]);
        driver.setClusterOrder([{ id: 1 }]);
        let res = driver.builtCluster(1);
        expect(res).toBe("TEST_TITLE");
    });
});

test('gets an update when ref changes', () => {
    withDriver({}, driver => {
        oneOneOne(driver);
        let updates = driver.batchedUpdates();
        expect(updates.clusters).toContainEqual([1, "TEST_TITLE"]);
        driver.insertReference({ id: "citekey", type: "book", title: "TEST_TITLE_2" });
        updates = driver.batchedUpdates();
        expect(updates.clusters).toContainEqual([1, "TEST_TITLE_2"]);
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
        let full = driver.fullRender();
        expect(full.allClusters).toHaveProperty("1", "TEST_TITLE");
        expect(full.bibEntries).toContainEqual({ id: "citekey", value: "TEST_TITLE" });
    })
});

test('update queue generally', () => {
    withDriver({style: bibStyle}, driver => {
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
        expect(driver.builtCluster(1)).toBe("ALTERED");

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
        driver.insertCluster({ id: 123, cites: [{ id: "added" }] });
        once = driver.batchedUpdates(); twice = driver.batchedUpdates();
        expect(once).toEqual(twice); checkUpdatesLen(once, 0, 0);

        // Add it to the document, and it's a different story
        driver.setClusterOrder([ {id:1},{id:123} ]);
        once = driver.batchedUpdates(); twice = driver.batchedUpdates();
        expect(once).not.toEqual(twice); checkUpdatesLen(twice, 0, 0);
        expect(once.clusters).toContainEqual([123, "ADDED"]);
        expect(once.bibliography?.entryIds).toEqual(["citekey", "added"]);
        expect(once.bibliography?.updatedEntries).toHaveProperty("added", "ADDED");
    })
})
