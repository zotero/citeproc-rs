// selenium-webdriver tests

if (process.env.MUST_RUN_FIREFOX_TESTS == "1" && !global.browser) {
    throw new Error(`cannot run firefox tests, probably because no FIREFOX_BINARY_PATH provided`)
}

var suite = describe.skip;
if (global.browser) {
    suite = describe;
}
// firefox could be first-booting itself. Give it time.
jest.setTimeout(60000);

suite("Integration tests for Firefox ESR 60.9", () => {
    test("no-modules build works end to end", async () => {
        await browser.get("file://" + __dirname + '/index.html');
        let fail = browser.wait(until.elementLocated(By.id('failure')), 10000);
        let succ = browser.wait(until.elementLocated(By.id('success')), 10000);
        let el = await Promise.race([fail, succ]);
        let id = await el.getAttribute("id");
        let text = await el.getText();
        expect(text).toBe("success");
        expect(id).toBe("success");
    })
})
