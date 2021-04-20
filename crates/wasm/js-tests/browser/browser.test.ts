

// selenium-webdriver tests
if (global.browser) {
    test("runs in firefox esr 60.9", async () => {
        jest.setTimeout(30000);
        const fs = require('fs');
        await browser.get("file://" + __dirname + '/index.html');
        let fail = browser.wait(until.elementLocated(By.id('failure')), 10000);
        let succ = browser.wait(until.elementLocated(By.id('success')), 10000);
        let el = await Promise.race([fail, succ]);
        let id = await el.getAttribute("id");
        let text = await el.getText();
        expect(text).toBe("success");
        expect(id).toBe("success");
        // throw new Error(para)
    })
}
