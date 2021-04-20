var firefoxBinary = "/Applications/Firefox ESR 60.9.app/Contents/MacOS/firefox-bin";
if (process.env.FIREFOX_BINARY_PATH) {
    firefoxBinary = process.env.FIREFOX_BINARY_PATH;
}
module.exports = {
    testEnvironment: "webdriver-environment",
    testEnvironmentOptions: {
        "browser": "firefox",
        options: {
            "moz:firefoxOptions": {
                "binary": firefoxBinary,
                "prefs": { },
                "args": [ "-headless" ]
            },
            "goog:chromeOptions": {
                "args": [ ]
            }
        }
    },
}
