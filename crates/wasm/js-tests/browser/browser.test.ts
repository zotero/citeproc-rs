

// selenium-webdriver tests
if (global.browser) {
    test("runs in firefox esr 60.9", async () => {
        jest.setTimeout(30000);
        const fs = require('fs');
        // the function passed in here is stringified and sent to the browser to execute.
        // let result = await browser.executeAsyncScript(function () {
        //     let callback = arguments[arguments.length - 1];
        //     let fn = async () => {
        //     };
        // })
        await browser.get("file://" + __dirname + '/index.html');
        let fail = browser.wait(until.elementLocated(By.id('failure')), 10000);
        let succ = browser.wait(until.elementLocated(By.id('success')), 10000);
        await Promise.all([fail, succ]);
        // throw new Error(para)
    })
}
