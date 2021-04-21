const fs = require('fs');

var firefoxBinary = null;

if (process.env.FIREFOX_BINARY_PATH) {
    firefoxBinary = process.env.FIREFOX_BINARY_PATH;
}

// For a detailed explanation regarding each configuration property, visit:
// https://jestjs.io/docs/en/configuration.html
var config = {
    // browser tests will self-ignore without webdriver stuff present
    roots: ["node", "browser"],
};

if (typeof firefoxBinary === "string" && fs.existsSync(firefoxBinary)) {
    config = {
        ...config,
        testEnvironment: "webdriver-environment",
        testEnvironmentOptions: {
            "browser": "firefox",
            options: {
                "moz:firefoxOptions": {
                    "binary": firefoxBinary,
                    "prefs": {
                        // make sure this very old ESR does not self-update!
                        "app.update.auto": false,
                    },
                    "args": [ "-headless" ]
                },
                "goog:chromeOptions": {
                    "args": [ ]
                }
            }
        }
    }
}

module.exports = config;
